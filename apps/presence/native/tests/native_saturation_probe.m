// Compara píxeles de capturas regionales: saturación 200/0/200/0/200.
// Las propiedades de la capa por sí solas no prueban el resultado visible.
#import "../face_wallpaper.m"
@interface GennieBlurProbePattern : NSView @end
@implementation GennieBlurProbePattern
- (void)drawRect:(NSRect)dirtyRect {
    (void)dirtyRect;
    for (int x = 0; x < 600; x += 12) {
        [[(x / 12) % 2 ? NSColor.systemRedColor : NSColor.systemBlueColor colorUsingColorSpace:NSColorSpace.sRGBColorSpace] setFill];
        NSRectFill(NSMakeRect(x, 0, 12, 400));
    }
}
@end
@interface GennieNativeBlurProbe : GennieWallpaperFace @end
@implementation GennieNativeBlurProbe
- (CGFloat)windowCoverageAt:(NSRect)face { (void)face; return 1; }
@end

static double chroma[5];
static void capture(NSString *prefix, int radius) {
    if (!prefix) return;
    NSTask *task = [NSTask new];
    task.launchPath = @"/usr/sbin/screencapture";
    task.arguments = @[@"-x", @"-R", @"800,552,128,128", [NSString stringWithFormat:@"%@-%d.png", prefix, radius]];
    [task launch];
    [task waitUntilExit];
    NSBitmapImageRep *pixels = [NSBitmapImageRep imageRepWithData:[NSData dataWithContentsOfFile:task.arguments.lastObject]];
    double sum = 0; int count = 0;
    for (int y=32; y<96; y++) for (int x=32; x<96; x++) {
        NSColor *c = [[pixels colorAtX:x y:y] colorUsingColorSpace:NSColorSpace.sRGBColorSpace];
        sum += fmax(c.redComponent, fmax(c.greenComponent, c.blueComponent)) - fmin(c.redComponent, fmin(c.greenComponent,c.blueComponent)); count++;
    }
    chroma[radius] = sum/count;
    printf("stage=%d rendered_chroma=%.6f\n", radius, chroma[radius]);
}

int main(int argc, const char *argv[]) {
    @autoreleasepool {
        NSString *prefix = argc == 3 && strcmp(argv[1], "--capture") == 0 ? [NSString stringWithUTF8String:argv[2]] : nil;
        [NSApplication sharedApplication];
        [NSApp setActivationPolicy:NSApplicationActivationPolicyAccessory];
        NSWindow *background = [[NSWindow alloc] initWithContentRect:NSMakeRect(700, 300, 600, 400)
            styleMask:NSWindowStyleMaskBorderless backing:NSBackingStoreBuffered defer:NO];
        background.releasedWhenClosed = NO;
        background.contentView = [GennieBlurProbePattern new];
        [background orderFrontRegardless];
        NSWindow *window = [[NSWindow alloc] initWithContentRect:NSMakeRect(800, 400, 128, 128)
            styleMask:NSWindowStyleMaskBorderless backing:NSBackingStoreBuffered defer:NO];
        window.releasedWhenClosed = NO;
        window.opaque = NO;
        window.backgroundColor = NSColor.clearColor;
        window.level = NSFloatingWindowLevel;
        NSView *container = [[NSView alloc] initWithFrame:NSMakeRect(0, 0, 128, 128)];
        NSView *glass = [[NSClassFromString(@"NSGlassEffectView") alloc] initWithFrame:container.bounds];
        [glass setValue:@64 forKey:@"cornerRadius"];
        NSBox *content = [[NSBox alloc] initWithFrame:container.bounds];
        content.boxType = NSBoxCustom;
        content.titlePosition = NSNoTitle;
        content.borderWidth = 0;
        content.cornerRadius = 64;
        content.fillColor = [NSColor colorWithSRGBRed:0.10 green:0.11 blue:0.12 alpha:0.24];
        [glass setValue:content forKey:@"contentView"];
        [container addSubview:glass];
        window.contentView = container;
        GennieNativeBlurProbe *face = [[GennieNativeBlurProbe alloc] initWithContainer:container glass:glass];
        [window orderFrontRegardless];
        __block NSUInteger stage = 0;
        __block BOOL passed = YES;
        static const int radii[] = {0, 0, 0, 0, 0};
        __block NSUInteger frame = 0;
        NSTimer *breathing = [NSTimer scheduledTimerWithTimeInterval:0.016 repeats:YES block:^(NSTimer *timer) {
            (void)timer;
            CGFloat diameter = 128 + sin(frame++ * 0.1) * 0.8;
            glass.frame = NSMakeRect(0, 0, diameter, diameter);
            [glass setValue:@(diameter / 2) forKey:@"cornerRadius"];
            content.frame = glass.bounds;
            content.cornerRadius = diameter / 2;
            [face updateForWindow:window localFace:glass.frame];
        }];
        [NSTimer scheduledTimerWithTimeInterval:0.25 repeats:YES block:^(NSTimer *timer) {
            if (stage > 0) {
                CALayer *layer = GennieGlassBlurLayer(glass.layer, 0);
                CALayer *presented = layer.presentationLayer ?: layer;
                NSNumber *actual = [presented valueForKeyPath:@"filters.glassBackground.inputBlurRadius"];
                int expected = radii[stage - 1];
                BOOL matches = actual && fabs(actual.doubleValue - expected) < 0.001;
                passed &= matches && face.nativeBlurSupported && face.blurRadius == expected;
                NSNumber *sat = [presented valueForKeyPath:@"filters.glassBackground.inputFaceColorMatrixSaturation"];
                NSNumber *white = [presented valueForKeyPath:@"filters.glassBackground.inputFaceColorMatrixWhite"];
                NSNumber *black = [presented valueForKeyPath:@"filters.glassBackground.inputFaceColorMatrixBlack"];
                passed &= sat && fabs(sat.doubleValue - 1.2 * face.saturation) < 0.001;
                passed &= white && fabs(white.doubleValue - (0.96 + face.brightness)) < 0.001;
                passed &= black && fabs(black.doubleValue - (0.4 + face.brightness)) < 0.001;
                printf("requested=%d configured=%.0f native=%s supported=%d\n", expected, face.blurRadius,
                    actual.description.UTF8String ?: "missing", face.nativeBlurSupported);
                capture(prefix, (int)stage-1);
            }
            if (stage == 5) {
                [timer invalidate];
                [breathing invalidate];
                passed &= chroma[0] - chroma[1] > 0.05;
                printf("saturation_visible=%s\n", passed ? "true" : "false");
                [window orderOut:nil];
                [background orderOut:nil];
                exit(passed ? 0 : 1);
            }
            gennie_face_wallpaper_configure((__bridge void *)face, radii[stage], stage % 2 ? 0 : 2, 0, 0.75, 0.33, 0.12);
            // Reproducir cambio de geometría como en la respiración de la cara.
            [glass setValue:@(64.0 + (stage % 2) * 0.5) forKey:@"cornerRadius"];
            [face updateForWindow:window localFace:NSMakeRect(0, 0, 128, 128)];
            stage++;
        }];
        [NSApp run];
        [window orderOut:nil];
        [background orderOut:nil];
        return passed ? 0 : 1;
    }
}

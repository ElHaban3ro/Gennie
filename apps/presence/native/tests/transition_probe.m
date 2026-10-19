// Sonda del cambio real de opacidad; sin DB, audio, red ni captura de pantalla.
#import "../face_wallpaper.m"
@interface TransitionFace : SOCWallpaperFace
@property NSUInteger coverageQueries;
@end
@implementation TransitionFace
- (CGFloat)windowCoverageAt:(NSRect)face { (void)face; self.coverageQueries++; return 0; }
@end
int main(void) {
    @autoreleasepool {
        [NSApplication sharedApplication];
        [NSApp setActivationPolicy:NSApplicationActivationPolicyAccessory];
        NSWindow *window = [[NSWindow alloc] initWithContentRect:NSMakeRect(700, 300, 226, 226)
            styleMask:NSWindowStyleMaskBorderless backing:NSBackingStoreBuffered defer:NO];
        window.releasedWhenClosed = NO;
        NSView *container = [[NSView alloc] initWithFrame:NSMakeRect(0, 0, 226, 226)];
        NSView *glass = [[NSClassFromString(@"NSGlassEffectView") alloc] initWithFrame:NSMakeRect(0, 0, 128, 128)];
        [container addSubview:glass];
        window.contentView = container;
        TransitionFace *face = [[TransitionFace alloc] initWithContainer:container glass:glass];
        BOOL passed = YES;
        // El helper anterior consulta cobertura y superpone una imagen.
        // El material global debe conservar la misma vista en cada posición.
        for (int x = 0; x < 400; x += 20) {
            [window setFrameOrigin:NSMakePoint(700 + x, 300)];
            [face updateForWindow:window localFace:NSMakeRect(0, 0, 128, 128)];
            passed &= !glass.hidden && glass.alphaValue == 1 && face.coverageQueries == 0;
            passed &= ![face.view isKindOfClass:NSImageView.class];
            passed &= NSEqualRects(face.view.frame, glass.frame);
        }
        printf("one_native_material_without_coverage_switch=%s queries=%lu\n", passed ? "true" : "false", face.coverageQueries);
        [window orderOut:nil];
        return passed ? 0 : 1;
    }
}

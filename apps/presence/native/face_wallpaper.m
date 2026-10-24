#import <AppKit/AppKit.h>
#import <CoreGraphics/CoreGraphics.h>
#import <QuartzCore/QuartzCore.h>

// NSGlassEffectView no publica el radio. Esta ruta interna se detecta en
// ejecución y solo modifica el filtro de nuestra propia vista de vidrio.
static CALayer *GennieGlassBlurLayer(CALayer *layer, NSUInteger depth) {
    if (!layer || depth > 24) return nil;
    @try {
        for (id filter in layer.filters) {
            if ([[filter valueForKey:@"name"] isEqual:@"glassBackground"] &&
                [[filter valueForKey:@"inputKeys"] containsObject:@"inputBlurRadius"]) return layer;
        }
    } @catch (NSException *exception) { (void)exception; }
    for (CALayer *child in layer.sublayers) {
        CALayer *result = GennieGlassBlurLayer(child, depth + 1);
        if (result) return result;
    }
    return nil;
}

static BOOL GennieLayerBelongsTo(CALayer *layer, CALayer *root) {
    for (NSUInteger depth = 0; layer && depth < 32; depth++, layer = layer.superlayer) {
        if (layer == root) return YES;
    }
    return NO;
}

// Un solo vidrio nativo: nunca se selecciona otro material según el fondo.
// Los nombres C históricos se conservan para no cambiar el puente Rust.
@interface GennieWallpaperFace : NSObject
@property (nonatomic, strong) NSView *view;
@property (nonatomic, strong) NSView *glass;
@property CGFloat blurRadius;
@property CGFloat saturation;
@property CGFloat brightness;
@property CGFloat transitionDuration;
@property (nonatomic, weak) CALayer *nativeBlurLayer;
@property BOOL nativeBlurSupported;
@end

@implementation GennieWallpaperFace
- (instancetype)initWithContainer:(NSView *)container glass:(NSView *)glass {
    self = [super init];
    if (!self) return nil;
    _glass = glass;
    _blurRadius = 6;
    _saturation = 1;
    _transitionDuration = 0.12;
    _view = [[NSView alloc] initWithFrame:NSZeroRect];
    _view.wantsLayer = YES;
    _view.layer.masksToBounds = YES;
    _view.layer.borderWidth = 0.75;
    _view.layer.borderColor = [NSColor colorWithWhite:1 alpha:0.33].CGColor;
    [container addSubview:_view];
    return self;
}
- (void)dealloc { [_view removeFromSuperview]; }

// AppKit reescribe el modelo al respirar. La animación conserva el valor en
// presentación; solo corre durante una edición y después mantiene su destino.
- (BOOL)holdParameter:(NSString *)parameter value:(CGFloat)value {
    NSString *key = [@"filters.glassBackground." stringByAppendingString:parameter];
    NSNumber *current = [_nativeBlurLayer valueForKeyPath:key];
    if (![current isKindOfClass:NSNumber.class]) return NO;
    NSString *name = [@"GennieMaterial." stringByAppendingString:parameter];
    CABasicAnimation *held = (CABasicAnimation *)[_nativeBlurLayer animationForKey:name];
    if (![held isKindOfClass:CABasicAnimation.class] || fabs([held.toValue doubleValue] - value) > 0.001) {
        NSNumber *presented = [_nativeBlurLayer.presentationLayer valueForKeyPath:key] ?: current;
        CABasicAnimation *hold = [CABasicAnimation animationWithKeyPath:key];
        hold.fromValue = _transitionDuration > 0 ? presented : @(value);
        hold.toValue = @(value);
        hold.duration = _transitionDuration > 0 ? _transitionDuration : 1;
        hold.fillMode = kCAFillModeForwards;
        hold.removedOnCompletion = NO;
        if (_transitionDuration <= 0) { hold.speed = 0; hold.timeOffset = 0.5; }
        [CATransaction begin];
        [CATransaction setDisableActions:YES];
        @try {
            [_nativeBlurLayer setValue:@(value) forKeyPath:key];
            [_nativeBlurLayer addAnimation:hold forKey:name];
        } @finally { [CATransaction commit]; }
    }
    return YES;
}

- (void)updateForWindow:(NSWindow *)window localFace:(NSRect)localFace {
    (void)window;
    CALayer *root = _glass.layer;
    if (!GennieLayerBelongsTo(_nativeBlurLayer, root)) _nativeBlurLayer = GennieGlassBlurLayer(root, 0);
    _nativeBlurSupported = NO;
    if (_nativeBlurLayer) {
        @try {
            _nativeBlurSupported = [self holdParameter:@"inputBlurRadius" value:_blurRadius];
            [self holdParameter:@"inputFaceColorMatrixSaturation" value:1.2 * _saturation];
            [self holdParameter:@"inputFaceColorMatrixWhite" value:fmax(0, 0.96 + _brightness)];
            [self holdParameter:@"inputFaceColorMatrixBlack" value:fmax(0, 0.4 + _brightness)];
        } @catch (NSException *exception) {
            (void)exception;
            _nativeBlurSupported = NO;
            _nativeBlurLayer = nil;
        }
    }
    [CATransaction begin];
    [CATransaction setDisableActions:YES];
    _view.frame = localFace;
    _view.layer.cornerRadius = NSWidth(localFace) / 2;
    [CATransaction commit];
}
@end

void *gennie_face_wallpaper_create(NSView *container, NSView *glass) {
    if (!container || !glass) return NULL;
    return (__bridge_retained void *)[[GennieWallpaperFace alloc] initWithContainer:container glass:glass];
}

void gennie_face_wallpaper_update(void *pointer, NSWindow *window,
                                double x, double y, double diameter) {
    if (!pointer || !window || diameter <= 0) return;
    GennieWallpaperFace *material = (__bridge GennieWallpaperFace *)pointer;
    [material updateForWindow:window localFace:NSMakeRect(x, y, diameter, diameter)];
}

void gennie_face_wallpaper_destroy(void *pointer) {
    if (pointer) CFBridgingRelease(pointer);
}

int gennie_face_wallpaper_native_blur_supported(void *pointer) {
    return pointer && ((__bridge GennieWallpaperFace *)pointer).nativeBlurSupported;
}

void gennie_face_wallpaper_configure(void *pointer, double blur, double saturation,
                                  double brightness, double borderWidth,
                                  double borderOpacity, double transition) {
    if (!pointer) return;
    GennieWallpaperFace *material = (__bridge GennieWallpaperFace *)pointer;
    material.blurRadius = blur;
    material.saturation = saturation;
    material.brightness = brightness;
    material.transitionDuration = transition;
    material.view.layer.borderWidth = borderWidth;
    material.view.layer.borderColor = [NSColor colorWithWhite:1 alpha:borderOpacity].CGColor;
}

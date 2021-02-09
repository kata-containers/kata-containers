// Copyright © 2015; Connor Hilarides
// Licensed under the MIT License <LICENSE.md>
//! Mappings for the contents of wincodec.h
pub type WICColor = ::UINT32;
pub type WICInProcPointer = *mut ::BYTE;
pub type REFWICPixelFormatGUID = ::REFGUID;
pub type WICPixelFormatGUID = ::GUID;
STRUCT!{struct WICRect {
    X: ::INT,
    Y: ::INT,
    Width: ::INT,
    Height: ::INT,
}}
ENUM!{enum WICColorContextType {
    WICColorContextUninitialized = 0,
    WICColorContextProfile = 0x1,
    WICColorContextExifColorSpace = 0x2,
}}
ENUM!{enum WICBitmapCreateCacheOption {
    WICBitmapNoCache = 0,
    WICBitmapCacheOnDemand = 0x1,
    WICBitmapCacheOnLoad = 0x2,
}}
ENUM!{enum WICDecodeOptions {
    WICDecodeMetadataCacheOnDemand = 0,
    WICDecodeMetadataCacheOnLoad = 0x1,
}}
ENUM!{enum WICBitmapEncoderCacheOption {
    WICBitmapEncoderCacheInMemory = 0,
    WICBitmapEncoderCacheTempFile = 0x1,
    WICBitmapEncoderNoCache = 0x2,
}}
FLAGS!{enum WICComponentType {
    WICDecoder = 0x1,
    WICEncoder = 0x2,
    WICPixelFormatConverter = 0x4,
    WICMetadataReader = 0x8,
    WICMetadataWriter = 0x10,
    WICPixelFormat = 0x20,
    WICAllComponents = 0x3f,
}}
FLAGS!{enum WICComponentEnumerateOptions {
    WICComponentEnumerateDefault = 0,
    WICComponentEnumerateRefresh = 0x1,
    WICComponentEnumerateDisabled = 0x80000000,
    WICComponentEnumerateUnsigned = 0x40000000,
    WICComponentEnumerateBuiltInOnly = 0x20000000,
}}
#[allow(unused_qualifications)]
STRUCT!{struct WICBitmapPattern {
    Position: ::ULARGE_INTEGER,
    Length: ::ULONG,
    Pattern: *mut ::BYTE,
    Mask: *mut ::BYTE,
    EndOfStream: ::BOOL,
}}
ENUM!{enum WICBitmapInterpolationMode {
    WICBitmapInterpolationModeNearestNeighbor = 0,
    WICBitmapInterpolationModeLinear = 0x1,
    WICBitmapInterpolationModeCubic = 0x2,
    WICBitmapInterpolationModeFant = 0x3,
}}
ENUM!{enum WICBitmapPaletteType {
    WICBitmapPaletteTypeCustom = 0,
    WICBitmapPaletteTypeMedianCut = 0x1,
    WICBitmapPaletteTypeFixedBW = 0x2,
    WICBitmapPaletteTypeFixedHalftone8 = 0x3,
    WICBitmapPaletteTypeFixedHalftone27 = 0x4,
    WICBitmapPaletteTypeFixedHalftone64 = 0x5,
    WICBitmapPaletteTypeFixedHalftone125 = 0x6,
    WICBitmapPaletteTypeFixedHalftone216 = 0x7,
    WICBitmapPaletteTypeFixedHalftone252 = 0x8,
    WICBitmapPaletteTypeFixedHalftone256 = 0x9,
    WICBitmapPaletteTypeFixedGray4 = 0xa,
    WICBitmapPaletteTypeFixedGray16 = 0xb,
    WICBitmapPaletteTypeFixedGray256 = 0xc,
}}
pub const WICBitmapPaletteTypeFixedWebPalette: WICBitmapPaletteType =
    WICBitmapPaletteTypeFixedHalftone216;
ENUM!{enum WICBitmapDitherType {
    WICBitmapDitherTypeSolid = 0,
    WICBitmapDitherTypeOrdered4x4 = 0x1,
    WICBitmapDitherTypeOrdered8x8 = 0x2,
    WICBitmapDitherTypeOrdered16x16 = 0x3,
    WICBitmapDitherTypeSpiral4x4 = 0x4,
    WICBitmapDitherTypeSpiral8x8 = 0x5,
    WICBitmapDitherTypeDualSpiral4x4 = 0x6,
    WICBitmapDitherTypeDualSpiral8x8 = 0x7,
    WICBitmapDitherTypeErrorDiffusion = 0x8,
}}
pub const WICBitmapDitherTypeNone: WICBitmapDitherType = WICBitmapDitherTypeSolid;
ENUM!{enum WICBitmapAlphaChannelOption {
    WICBitmapUseAlpha = 0,
    WICBitmapUsePremultipliedAlpha = 0x1,
    WICBitmapIgnoreAlpha = 0x2,
}}
FLAGS!{enum WICBitmapTransformOptions {
    WICBitmapTransformRotate0 = 0,
    WICBitmapTransformRotate90 = 0x1,
    WICBitmapTransformRotate180 = 0x2,
    WICBitmapTransformRotate270 = 0x3,
    WICBitmapTransformFlipHorizontal = 0x8,
    WICBitmapTransformFlipVertical = 0x10,
}}
FLAGS!{enum WICBitmapLockFlags {
    WICBitmapLockRead = 0x1,
    WICBitmapLockWrite = 0x2,
}}
FLAGS!{enum WICBitmapDecoderCapabilities {
    WICBitmapDecoderCapabilitySameEncoder = 0x1,
    WICBitmapDecoderCapabilityCanDecodeAllImages = 0x2,
    WICBitmapDecoderCapabilityCanDecodeSomeImages = 0x4,
    WICBitmapDecoderCapabilityCanEnumerateMetadata = 0x8,
    WICBitmapDecoderCapabilityCanDecodeThumbnail = 0x10,
}}
FLAGS!{enum WICProgressOperation {
    WICProgressOperationCopyPixels = 0x1,
    WICProgressOperationWritePixels = 0x2,
    WICProgressOperationAll = 0xffff,
}}
FLAGS!{enum WICProgressNotification {
    WICProgressNotificationBegin = 0x10000,
    WICProgressNotificationEnd = 0x20000,
    WICProgressNotificationFrequent = 0x40000,
    WICProgressNotificationAll = 0xffff0000,
}}
FLAGS!{enum WICComponentSigning {
    WICComponentSigned = 0x1,
    WICComponentUnsigned = 0x2,
    WICComponentSafe = 0x4,
    WICComponentDisabled = 0x80000000,
}}
ENUM!{enum WICGifLogicalScreenDescriptorProperties {
    WICGifLogicalScreenSignature = 0x1,
    WICGifLogicalScreenDescriptorWidth = 0x2,
    WICGifLogicalScreenDescriptorHeight = 0x3,
    WICGifLogicalScreenDescriptorGlobalColorTableFlag = 0x4,
    WICGifLogicalScreenDescriptorColorResolution = 0x5,
    WICGifLogicalScreenDescriptorSortFlag = 0x6,
    WICGifLogicalScreenDescriptorGlobalColorTableSize = 0x7,
    WICGifLogicalScreenDescriptorBackgroundColorIndex = 0x8,
    WICGifLogicalScreenDescriptorPixelAspectRatio = 0x9,
}}
ENUM!{enum WICGifImageDescriptorProperties {
    WICGifImageDescriptorLeft = 0x1,
    WICGifImageDescriptorTop = 0x2,
    WICGifImageDescriptorWidth = 0x3,
    WICGifImageDescriptorHeight = 0x4,
    WICGifImageDescriptorLocalColorTableFlag = 0x5,
    WICGifImageDescriptorInterlaceFlag = 0x6,
    WICGifImageDescriptorSortFlag = 0x7,
    WICGifImageDescriptorLocalColorTableSize = 0x8,
}}
ENUM!{enum WICGifGraphicControlExtensionProperties {
    WICGifGraphicControlExtensionDisposal = 0x1,
    WICGifGraphicControlExtensionUserInputFlag = 0x2,
    WICGifGraphicControlExtensionTransparencyFlag = 0x3,
    WICGifGraphicControlExtensionDelay = 0x4,
    WICGifGraphicControlExtensionTransparentColorIndex = 0x5,
}}
ENUM!{enum WICGifApplicationExtensionProperties {
    WICGifApplicationExtensionApplication = 0x1,
    WICGifApplicationExtensionData = 0x2,
}}
ENUM!{enum WICGifCommentExtensionProperties {
    WICGifCommentExtensionText = 0x1,
}}
ENUM!{enum WICJpegCommentProperties {
    WICJpegCommentText = 0x1,
}}
ENUM!{enum WICJpegLuminanceProperties {
    WICJpegLuminanceTable = 0x1,
}}
ENUM!{enum WICJpegChrominanceProperties {
    WICJpegChrominanceTable = 0x1,
}}
ENUM!{enum WIC8BIMIptcProperties {
    WIC8BIMIptcPString = 0,
    WIC8BIMIptcEmbeddedIPTC = 0x1,
}}
ENUM!{enum WIC8BIMResolutionInfoProperties {
    WIC8BIMResolutionInfoPString = 0x1,
    WIC8BIMResolutionInfoHResolution = 0x2,
    WIC8BIMResolutionInfoHResolutionUnit = 0x3,
    WIC8BIMResolutionInfoWidthUnit = 0x4,
    WIC8BIMResolutionInfoVResolution = 0x5,
    WIC8BIMResolutionInfoVResolutionUnit = 0x6,
    WIC8BIMResolutionInfoHeightUnit = 0x7,
}}
ENUM!{enum WIC8BIMIptcDigestProperties {
    WIC8BIMIptcDigestPString = 0x1,
    WIC8BIMIptcDigestIptcDigest = 0x2,
}}
ENUM!{enum WICPngGamaProperties {
    WICPngGamaGamma = 0x1,
}}
ENUM!{enum WICPngBkgdProperties {
    WICPngBkgdBackgroundColor = 0x1,
}}
ENUM!{enum WICPngItxtProperties {
    WICPngItxtKeyword = 0x1,
    WICPngItxtCompressionFlag = 0x2,
    WICPngItxtLanguageTag = 0x3,
    WICPngItxtTranslatedKeyword = 0x4,
    WICPngItxtText = 0x5,
}}
ENUM!{enum WICPngChrmProperties {
    WICPngChrmWhitePointX = 0x1,
    WICPngChrmWhitePointY = 0x2,
    WICPngChrmRedX = 0x3,
    WICPngChrmRedY = 0x4,
    WICPngChrmGreenX = 0x5,
    WICPngChrmGreenY = 0x6,
    WICPngChrmBlueX = 0x7,
    WICPngChrmBlueY = 0x8,
}}
ENUM!{enum WICPngHistProperties {
    WICPngHistFrequencies = 0x1,
}}
ENUM!{enum WICPngIccpProperties {
    WICPngIccpProfileName = 0x1,
    WICPngIccpProfileData = 0x2,
}}
ENUM!{enum WICPngSrgbProperties {
    WICPngSrgbRenderingIntent = 0x1,
}}
ENUM!{enum WICPngTimeProperties {
    WICPngTimeYear = 0x1,
    WICPngTimeMonth = 0x2,
    WICPngTimeDay = 0x3,
    WICPngTimeHour = 0x4,
    WICPngTimeMinute = 0x5,
    WICPngTimeSecond = 0x6,
}}
ENUM!{enum WICSectionAccessLevel {
    WICSectionAccessLevelRead = 0x1,
    WICSectionAccessLevelReadWrite = 0x3,
}}
ENUM!{enum WICPixelFormatNumericRepresentation {
    WICPixelFormatNumericRepresentationUnspecified = 0,
    WICPixelFormatNumericRepresentationIndexed = 0x1,
    WICPixelFormatNumericRepresentationUnsignedInteger = 0x2,
    WICPixelFormatNumericRepresentationSignedInteger = 0x3,
    WICPixelFormatNumericRepresentationFixed = 0x4,
    WICPixelFormatNumericRepresentationFloat = 0x5,
}}
ENUM!{enum WICPlanarOptions {
    WICPlanarOptionsDefault = 0,
    WICPlanarOptionsPreserveSubsampling = 0x1,
}}
#[allow(unused_qualifications)]
STRUCT!{struct WICImageParameters {
    PixelFormat: ::D2D1_PIXEL_FORMAT,
    DpiX: ::FLOAT,
    DpiY: ::FLOAT,
    Top: ::FLOAT,
    Left: ::FLOAT,
    PixelWidth: ::FLOAT,
    PixelHeight: ::FLOAT,
}}
#[allow(unused_qualifications)]
STRUCT!{struct WICBitmapPlaneDescription {
    Format: WICPixelFormatGUID,
    Width: ::UINT,
    Height: ::UINT,
}}
#[allow(unused_qualifications)]
STRUCT!{struct WICBitmapPlane {
    Format: WICPixelFormatGUID,
    pbBuffer: *mut ::BYTE,
    cbStride: ::UINT,
    cbBufferSize: ::UINT,
}}
RIDL!(
interface IWICPalette(IWICPaletteVtbl): IUnknown(IUnknownVtbl) {
    fn InitializePredefined(
        &mut self, ePaletteType: WICBitmapPaletteType, fAddTransparentColor: ::BOOL
    ) -> ::HRESULT,
    fn InitializeCustom(&mut self, pColors: *mut WICColor, cCount: ::UINT) -> ::HRESULT,
    fn InitializeFromBitmap(
        &mut self, pISurface: *mut IWICBitmapSource, cCount: ::UINT, fAddTransparentColor: ::BOOL
    ) -> ::HRESULT,
    fn InitializeFromPalette(&mut self, pIPalette: *mut IWICPalette) -> ::HRESULT,
    fn GetType(&mut self, pePaletteType: *mut WICBitmapPaletteType) -> ::HRESULT,
    fn GetColorCount(&mut self, pcCount: *mut ::UINT) -> ::HRESULT,
    fn GetColors(
        &mut self, cCount: ::UINT, pColros: *mut WICColor, pcActualColors: *mut ::UINT
    ) -> ::HRESULT,
    fn IsBlackWhite(&mut self, pfIsBlackWhite: *mut ::BOOL) -> ::HRESULT,
    fn IsGrayscale(&mut self, pfIsGrayscale: *mut ::BOOL) -> ::HRESULT,
    fn HasAlpha(&mut self, pfHasAlpha: *mut ::BOOL) -> ::HRESULT
});
RIDL!(
interface IWICBitmapSource(IWICBitmapSourceVtbl): IUnknown(IUnknownVtbl) {
    fn GetSize(&mut self, puiWidth: *mut ::UINT, puiHeight: ::UINT) -> ::HRESULT,
    fn GetPixelFormat(&mut self, pPixelFormat: *mut WICPixelFormatGUID) -> ::HRESULT,
    fn GetResolution(&mut self, pDpiX: *mut f64, pDpiY: *mut f64) -> ::HRESULT,
    fn CopyPalette(&mut self, pIPalette: *mut IWICPalette) -> ::HRESULT,
    fn CopyPixels(
        &mut self, prc: *const WICRect, cbStride: ::UINT, cbBufferSize: ::UINT,
        pbBuffer: *mut ::BYTE
    ) -> ::HRESULT
});
RIDL!(
interface IWICFormatConverter(IWICFormatConverterVtbl): IWICBitmapSource(IWICBitmapSourceVtbl) {
    fn Initialize(
        &mut self, pISource: *mut IWICBitmapSource, dstFormat: REFWICPixelFormatGUID,
        dither: WICBitmapDitherType, pIPalette: *mut IWICPalette, alphaThreasholdPercent: f64,
        paletteTranslate: WICBitmapPaletteType
    ) -> ::HRESULT,
    fn CanConvert(
        &mut self, srcPixelFormat: REFWICPixelFormatGUID, dstPixelFormat: REFWICPixelFormatGUID,
        pfCanConvert: *mut ::BOOL
    ) -> ::HRESULT
});
RIDL!(
interface IWICPlanarFormatConverter(IWICPlanarFormatConverterVtbl)
    : IWICBitmapSource(IWICBitmapSourceVtbl) {
    fn Initialize(
        &mut self, ppPlanes: *mut *mut IWICBitmapSource, cPlanes: ::UINT,
        dstFormat: REFWICPixelFormatGUID, dither: WICBitmapDitherType, pIPalette: *mut IWICPalette,
        alphaThreasholdPercent: f64, paletteTranslate: WICBitmapPaletteType
    ) -> ::HRESULT,
    fn CanConvert(
        &mut self, pSrcPixelFormats: *const WICPixelFormatGUID, cSrcPlanes: ::UINT,
        dstPixelFormat: REFWICPixelFormatGUID, pfCanConvert: *mut ::BOOL
    ) -> ::HRESULT
});
RIDL!(
interface IWICBitmapScaler(IWICBitmapScalerVtbl): IWICBitmapSource(IWICBitmapSourceVtbl) {
    fn Initialize(
        &mut self, pISource: *mut IWICBitmapSource, uiWidth: ::UINT, uiHeight: ::UINT,
        mode: WICBitmapInterpolationMode
    ) -> ::HRESULT
});
RIDL!(
interface IWICBitmapClipper(IWICBitmapClipperVtbl): IWICBitmapSource(IWICBitmapSourceVtbl) {
    fn Initialize(&mut self, pISource: *mut IWICBitmapSource, prc: *const WICRect) -> ::HRESULT
});
RIDL!(
interface IWICBitmapFlipRotator(IWICBitmapFlipRotatorVtbl)
    : IWICBitmapSource(IWICBitmapSourceVtbl) {
    fn Initialize(
        &mut self, pISource: *mut IWICBitmapSource, options: WICBitmapTransformOptions
    ) -> ::HRESULT
});
RIDL!(
interface IWICBitmapLock(IWICBitmapLockVtbl): IUnknown(IUnknownVtbl) {
    fn GetSize(&mut self, puiWidth: *mut ::UINT, puiHeight: *mut ::UINT) -> ::HRESULT,
    fn GetStride(&mut self, pcbStride: *mut ::UINT) -> ::HRESULT,
    fn GetDataPointer(
        &mut self, pcbBufferSize: *mut ::UINT, ppbData: *mut WICInProcPointer
    ) -> ::HRESULT,
    fn GetPixelFormat(&mut self, pPixelFormat: *mut WICPixelFormatGUID) -> ::HRESULT
});
RIDL!(
interface IWICBitmap(IWICBitmapVtbl): IWICBitmapSource(IWICBitmapSourceVtbl) {
    fn Lock(
        &mut self, prcLock: *const WICRect, flags: ::DWORD, ppILock: *mut *mut IWICBitmapLock
    ) -> ::HRESULT,
    fn SetPalette(&mut self, pIPalette: *mut IWICPalette) -> ::HRESULT,
    fn SetResolution(&mut self, dpiX: f64, dpiY: f64) -> ::HRESULT
});

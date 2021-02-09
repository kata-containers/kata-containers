// Copyright © 2015; Dmitry Roschin
// Licensed under the MIT License <LICENSE.md>
//! Mappings for the contents of dxgi1_2.h

ENUM!{ enum DXGI_ALPHA_MODE {
    DXGI_ALPHA_MODE_UNSPECIFIED = 0,
    DXGI_ALPHA_MODE_PREMULTIPLIED = 1,
    DXGI_ALPHA_MODE_STRAIGHT = 2,
    DXGI_ALPHA_MODE_IGNORE = 3,
    DXGI_ALPHA_MODE_FORCE_DWORD = 0xFFFFFFFF,
}}

ENUM!{ enum DXGI_COMPUTE_PREEMPTION_GRANULARITY {
    DXGI_COMPUTE_PREEMPTION_DMA_BUFFER_BOUNDARY = 0,
    DXGI_COMPUTE_PREEMPTION_DISPATCH_BOUNDARY = 1,
    DXGI_COMPUTE_PREEMPTION_THREAD_GROUP_BOUNDARY = 2,
    DXGI_COMPUTE_PREEMPTION_THREAD_BOUNDARY = 3,
    DXGI_COMPUTE_PREEMPTION_INSTRUCTION_BOUNDARY = 4,
}}

ENUM!{ enum DXGI_GRAPHICS_PREEMPTION_GRANULARITY {
    DXGI_GRAPHICS_PREEMPTION_DMA_BUFFER_BOUNDARY = 0,
    DXGI_GRAPHICS_PREEMPTION_PRIMITIVE_BOUNDARY = 1,
    DXGI_GRAPHICS_PREEMPTION_TRIANGLE_BOUNDARY = 2,
    DXGI_GRAPHICS_PREEMPTION_PIXEL_BOUNDARY = 3,
    DXGI_GRAPHICS_PREEMPTION_INSTRUCTION_BOUNDARY = 4,
}}

ENUM!{ enum DXGI_OUTDUPL_POINTER_SHAPE_TYPE {
    DXGI_OUTDUPL_POINTER_SHAPE_TYPE_MONOCHROME = 1,
    DXGI_OUTDUPL_POINTER_SHAPE_TYPE_COLOR = 2,
    DXGI_OUTDUPL_POINTER_SHAPE_TYPE_MASKED_COLOR = 4,
}}

ENUM!{ enum DXGI_SCALING {
    DXGI_SCALING_STRETCH = 0,
    DXGI_SCALING_NONE = 1,
    DXGI_SCALING_ASPECT_RATIO_STRETCH = 2,
}}

ENUM!{ enum _DXGI_OFFER_RESOURCE_PRIORITY {
    DXGI_OFFER_RESOURCE_PRIORITY_LOW = 1,
    DXGI_OFFER_RESOURCE_PRIORITY_NORMAL = 2,
    DXGI_OFFER_RESOURCE_PRIORITY_HIGH = 3,
}}

STRUCT!{nodebug struct DXGI_ADAPTER_DESC2 {
    Description: [::WCHAR; 128],
    VendorId: ::UINT,
    DeviceId: ::UINT,
    SubSysId: ::UINT,
    Revision: ::UINT,
    DedicatedVideoMemory: ::SIZE_T,
    DedicatedSystemMemory: ::SIZE_T,
    SharedSystemMemory: ::SIZE_T,
    AdapterLuid: ::LUID,
    Flags: ::UINT,
    GraphicsPreemptionGranularity: ::DXGI_GRAPHICS_PREEMPTION_GRANULARITY,
    ComputePreemptionGranularity: ::DXGI_COMPUTE_PREEMPTION_GRANULARITY,
}}

STRUCT!{struct DXGI_MODE_DESC1 {
    Width: ::UINT,
    Height: ::UINT,
    RefreshRate: ::DXGI_RATIONAL,
    Format: ::DXGI_FORMAT,
    ScanlineOrdering: ::DXGI_MODE_SCANLINE_ORDER,
    Scaling: ::DXGI_MODE_SCALING,
    Stereo: ::BOOL,
}}

STRUCT!{struct DXGI_OUTDUPL_DESC {
    ModeDesc: ::DXGI_MODE_DESC,
    Rotation: ::DXGI_MODE_ROTATION,
    DesktopImageInSystemMemory: ::BOOL,
}}

STRUCT!{struct DXGI_OUTDUPL_FRAME_INFO {
    LastPresentTime: ::LARGE_INTEGER,
    LastMouseUpdateTime: ::LARGE_INTEGER,
    AccumulatedFrames: ::UINT,
    RectsCoalesced: ::BOOL,
    ProtectedContentMaskedOut: ::BOOL,
    PointerPosition: ::DXGI_OUTDUPL_POINTER_POSITION,
    TotalMetadataBufferSize: ::UINT,
    PointerShapeBufferSize: ::UINT,
}}

STRUCT!{struct DXGI_OUTDUPL_MOVE_RECT {
    SourcePoint: ::POINT,
    DestinationRect: ::RECT,
}}

STRUCT!{struct DXGI_OUTDUPL_POINTER_POSITION {
    Position: ::POINT,
    Visible: ::BOOL,
}}

STRUCT!{struct DXGI_OUTDUPL_POINTER_SHAPE_INFO {
    Type: ::UINT,
    Width: ::UINT,
    Height: ::UINT,
    Pitch: ::UINT,
    HotSpot: ::POINT,
}}

STRUCT!{struct DXGI_PRESENT_PARAMETERS {
    DirtyRectsCount: ::UINT,
    pDirtyRects: *mut ::RECT,
    pScrollRect: *mut ::RECT,
    pScrollOffset: *mut ::POINT,
}}

STRUCT!{struct DXGI_SWAP_CHAIN_DESC1 {
    Width: ::UINT,
    Height: ::UINT,
    Format: ::DXGI_FORMAT,
    Stereo: ::BOOL,
    SampleDesc: ::DXGI_SAMPLE_DESC,
    BufferUsage: ::DXGI_USAGE,
    BufferCount: ::UINT,
    Scaling: ::DXGI_SCALING,
    SwapEffect: ::DXGI_SWAP_EFFECT,
    AlphaMode: ::DXGI_ALPHA_MODE,
    Flags: ::UINT,
}}

STRUCT!{struct DXGI_SWAP_CHAIN_FULLSCREEN_DESC {
    RefreshRate: ::DXGI_RATIONAL,
    ScanlineOrdering: ::DXGI_MODE_SCANLINE_ORDER,
    Scaling: ::DXGI_MODE_SCALING,
    Windowed: ::BOOL,
}}

RIDL!(
interface IDXGIAdapter2(IDXGIAdapter2Vtbl): IDXGIAdapter1(IDXGIAdapter1Vtbl) {
    fn GetDesc2(&mut self, pDesc: *mut ::DXGI_ADAPTER_DESC2) -> ::HRESULT
});

RIDL!(
interface IDXGIDevice2(IDXGIDevice2Vtbl): IDXGIDevice1(IDXGIDevice1Vtbl) {
    fn OfferResources(
        &mut self, NumResources: ::UINT, ppResources: *mut *mut ::IDXGIResource,
        Priority: ::DXGI_OFFER_RESOURCE_PRIORITY
    ) -> ::HRESULT,
    fn ReclaimResources(
        &mut self, NumResources: ::UINT, ppResources: *mut *mut ::IDXGIResource,
        pDiscarded: *mut ::BOOL
    ) -> ::HRESULT,
    fn EnqueueSetEvent(&mut self, hEvent: ::HANDLE) -> ::HRESULT
});

RIDL!(
interface IDXGIDisplayControl(IDXGIDisplayControlVtbl): IUnknown(IUnknownVtbl) {
    fn IsStereoEnabled(&mut self) -> ::BOOL,
    fn SetStereoEnabled(&mut self, enabled: ::BOOL) -> ()
});

RIDL!(
interface IDXGIFactory2(IDXGIFactory2Vtbl): IDXGIFactory1(IDXGIFactory1Vtbl) {
    fn IsWindowedStereoEnabled(&mut self) -> ::BOOL,
    fn CreateSwapChainForHwnd(
        &mut self, pDevice: *mut ::IUnknown, hWnd: ::HWND, pDesc: *const ::DXGI_SWAP_CHAIN_DESC1,
        pFullscreenDesc: *const ::DXGI_SWAP_CHAIN_FULLSCREEN_DESC,
        pRestrictToOutput: *mut ::IDXGIOutput, ppSwapChain: *mut *mut ::IDXGISwapChain1
    ) -> ::HRESULT,
    fn CreateSwapChainForCoreWindow(
        &mut self, pDevice: *mut ::IUnknown, pWindow: *mut ::IUnknown,
        pDesc: *const ::DXGI_SWAP_CHAIN_DESC1, pRestrictToOutput: *mut ::IDXGIOutput,
        ppSwapChain: *mut *mut ::IDXGISwapChain1
    ) -> ::HRESULT,
    fn GetSharedResourceAdapterLuid(
        &mut self, hResource: ::HANDLE, pLuid: *mut ::LUID
    ) -> ::HRESULT,
    fn RegisterStereoStatusWindow(
        &mut self, WindowHandle: ::HWND, wMsg: ::UINT, pdwCookie: *mut ::DWORD
    ) -> ::HRESULT,
    fn RegisterStereoStatusEvent(
        &mut self, hEvent: ::HANDLE, pdwCookie: *mut ::DWORD
    ) -> ::HRESULT,
    fn UnregisterStereoStatus(&mut self, dwCookie: ::DWORD) -> (),
    fn RegisterOcclusionStatusWindow(
        &mut self, WindowHandle: ::HWND, wMsg: ::UINT, pdwCookie: *mut ::DWORD
    ) -> ::HRESULT,
    fn RegisterOcclusionStatusEvent(
        &mut self, hEvent: ::HANDLE, pdwCookie: *mut ::DWORD
    ) -> ::HRESULT,
    fn UnregisterOcclusionStatus(&mut self, dwCookie: ::DWORD) -> (),
    fn CreateSwapChainForComposition(
        &mut self, pDevice: *mut ::IUnknown, pDesc: *const ::DXGI_SWAP_CHAIN_DESC1,
        pRestrictToOutput: *mut ::IDXGIOutput, ppSwapChain: *mut *mut ::IDXGISwapChain1
    ) -> ::HRESULT
});

RIDL!(
interface IDXGIOutput1(IDXGIOutput1Vtbl): IDXGIOutput(IDXGIOutputVtbl) {
    fn GetDisplayModeList1(
        &mut self, EnumFormat: ::DXGI_FORMAT, Flags: ::UINT, pNumModes: *mut ::UINT,
        pDesc: *mut ::DXGI_MODE_DESC1
    ) -> ::HRESULT,
    fn FindClosestMatchingMode1(
        &mut self, pModeToMatch: *const ::DXGI_MODE_DESC1, pClosestMatch: *mut ::DXGI_MODE_DESC1,
        pConcernedDevice: *mut ::IUnknown
    ) -> ::HRESULT,
    fn GetDisplaySurfaceData1(
        &mut self, pDestination: *mut ::IDXGIResource
    ) -> ::HRESULT,
    fn DuplicateOutput(
        &mut self, pDevice: *mut ::IUnknown,
        ppOutputDuplication: *mut *mut ::IDXGIOutputDuplication
    ) -> ::HRESULT
});

RIDL!(
interface IDXGIOutputDuplication(IDXGIOutputDuplicationVtbl): IDXGIObject(IDXGIObjectVtbl) {
    fn GetDesc(&mut self, pDesc: *mut ::DXGI_OUTDUPL_DESC) -> (),
    fn AcquireNextFrame(
        &mut self, TimeoutInMilliseconds: ::UINT, pFrameInfo: *mut ::DXGI_OUTDUPL_FRAME_INFO,
        ppDesktopResource: *mut *mut ::IDXGIResource
    ) -> ::HRESULT,
    fn GetFrameDirtyRects(
        &mut self, DirtyRectsBufferSize: ::UINT, pDirtyRectsBuffer: *mut ::RECT,
        pDirtyRectsBufferSizeRequired: *mut ::UINT
    ) -> ::HRESULT,
    fn GetFrameMoveRects(
        &mut self, MoveRectsBufferSize: ::UINT, pMoveRectBuffer: *mut ::DXGI_OUTDUPL_MOVE_RECT,
        pMoveRectsBufferSizeRequired: *mut ::UINT
    ) -> ::HRESULT,
    fn GetFramePointerShape(
        &mut self, PointerShapeBufferSize: ::UINT, pPointerShapeBuffer: *mut ::c_void,
        pPointerShapeBufferSizeRequired: *mut ::UINT,
        pPointerShapeInfo: *mut ::DXGI_OUTDUPL_POINTER_SHAPE_INFO
    ) -> ::HRESULT,
    fn MapDesktopSurface(
        &mut self, pLockedRect: *mut ::DXGI_MAPPED_RECT
    ) -> ::HRESULT,
    fn UnMapDesktopSurface(&mut self) -> ::HRESULT,
    fn ReleaseFrame(&mut self) -> ::HRESULT
});

RIDL!(
interface IDXGIResource1(IDXGIResource1Vtbl): IDXGIResource(IDXGIResourceVtbl) {
    fn CreateSubresourceSurface(
        &mut self, index: ::UINT, ppSurface: *mut *mut ::IDXGISurface2
    ) -> ::HRESULT,
    fn CreateSharedHandle(
        &mut self, pAttributes: *const ::SECURITY_ATTRIBUTES, dwAccess: ::DWORD, lpName: ::LPCWSTR,
        pHandle: *mut ::HANDLE
    ) -> ::HRESULT
});

RIDL!(
interface IDXGISurface2(IDXGISurface2Vtbl): IDXGISurface1(IDXGISurface1Vtbl) {
    fn GetResource(
        &mut self, riid: ::REFGUID, ppParentResource: *mut *mut ::c_void,
        pSubresourceIndex: *mut ::UINT
    ) -> ::HRESULT
});

RIDL!(
interface IDXGISwapChain1(IDXGISwapChain1Vtbl): IDXGISwapChain(IDXGISwapChainVtbl) {
    fn GetDesc1(&mut self, pDesc: *mut ::DXGI_SWAP_CHAIN_DESC1) -> ::HRESULT,
    fn GetFullscreenDesc(
        &mut self, pDesc: *mut ::DXGI_SWAP_CHAIN_FULLSCREEN_DESC
    ) -> ::HRESULT,
    fn GetHwnd(&mut self, pHwnd: *mut ::HWND) -> ::HRESULT,
    fn GetCoreWindow(
        &mut self, refiid: ::REFGUID, ppUnk: *mut *mut ::c_void
    ) -> ::HRESULT,
    fn Present1(
        &mut self, SyncInterval: ::UINT, PresentFlags: ::UINT,
        pPresentParameters: *const ::DXGI_PRESENT_PARAMETERS
    ) -> ::HRESULT,
    fn IsTemporaryMonoSupported(&mut self) -> ::BOOL,
    fn GetRestrictToOutput(
        &mut self, ppRestrictToOutput: *mut *mut ::IDXGIOutput
    ) -> ::HRESULT,
    fn SetBackgroundColor(&mut self, pColor: *const ::DXGI_RGBA) -> ::HRESULT,
    fn GetBackgroundColor(&mut self, pColor: *mut ::DXGI_RGBA) -> ::HRESULT,
    fn SetRotation(&mut self, Rotation: ::DXGI_MODE_ROTATION) -> ::HRESULT,
    fn GetRotation(&mut self, pRotation: *mut ::DXGI_MODE_ROTATION) -> ::HRESULT
});

pub type DXGI_OFFER_RESOURCE_PRIORITY = ::_DXGI_OFFER_RESOURCE_PRIORITY;
pub const DXGI_ENUM_MODES_DISABLED_STEREO: ::UINT = 8;
pub const DXGI_ENUM_MODES_STEREO: ::UINT = 4;
pub const DXGI_SHARED_RESOURCE_READ: ::UINT = 0x80000000;
pub const DXGI_SHARED_RESOURCE_WRITE: ::UINT = 1;

// Copyright © 2015, Corey Richardson
// Licensed under the MIT License <LICENSE.md>
//! Direct3D include file
pub const D3D_SDK_VERSION: ::DWORD = 32;
pub const D3D9b_SDK_VERSION: ::DWORD = 31;
RIDL!(
interface IDirect3D9(IDirect3D9Vtbl): IUnknown(IUnknownVtbl) {
    fn RegisterSoftwareDevice(&mut self, pInitializeFunction: *mut ::VOID) -> ::HRESULT,
    fn GetAdapterCount(&mut self) -> ::UINT,
    fn GetAdapterIdentifier(
        &mut self, Adapter: ::UINT, Flags: ::DWORD, pIdentifier: *mut ::D3DADAPTER_IDENTIFIER9
    ) -> ::HRESULT,
    fn GetAdapterModeCount(&mut self, Adapter: ::UINT, Format: ::D3DFORMAT) -> ::UINT,
    fn EnumAdapterModes(
        &mut self, Adapter: ::UINT, Format: ::D3DFORMAT, Mode: ::UINT, pMode: *mut ::D3DDISPLAYMODE
    ) -> ::HRESULT,
    fn GetAdapterDisplayMode(
        &mut self, Adapter: ::UINT, pMode: *mut ::D3DDISPLAYMODE
    ) -> ::HRESULT,
    fn CheckDeviceType(
        &mut self, Adapter: ::UINT, DevType: ::D3DDEVTYPE, AdapterFormat: ::D3DFORMAT,
        BackBufferFormat: ::D3DFORMAT, bWindowed: ::BOOL
    ) -> ::HRESULT,
    fn CheckDeviceFormat(
        &mut self, Adapter: ::UINT, DeviceType: ::D3DDEVTYPE, AdapterFormat: ::D3DFORMAT,
        Usage: ::DWORD, RType: ::D3DRESOURCETYPE, CheckFormat: ::D3DFORMAT
    ) -> ::HRESULT,
    fn CheckDeviceMultiSampleType(
        &mut self, Adapter: ::UINT, DeviceType: ::D3DDEVTYPE, SurfaceFormat: ::D3DFORMAT,
        Windowed: ::BOOL, MultiSampleType: ::D3DMULTISAMPLE_TYPE, pQualityLevels: *mut ::DWORD
    ) -> ::HRESULT,
    fn CheckDepthStencilMatch(
        &mut self, Adapter: ::UINT, DeviceType: ::D3DDEVTYPE, AdapterFormat: ::D3DFORMAT,
        RenderTargetFormat: ::D3DFORMAT, DepthStencilFormat: ::D3DFORMAT
    ) -> ::HRESULT,
    fn CheckDeviceFormatConversion(
        &mut self, Adapter: ::UINT, DeviceType: ::D3DDEVTYPE, SourceFormat: ::D3DFORMAT,
        TargetFormat: ::D3DFORMAT
    ) -> ::HRESULT,
    fn GetDeviceCaps(
        &mut self, Adapter: ::UINT, DeviceType: ::D3DDEVTYPE, pCaps: *mut ::D3DCAPS9
    ) -> ::HRESULT,
    fn GetAdapterMonitor(&mut self, Adapter: ::UINT) -> ::HMONITOR,
    fn CreateDevice(
        &mut self, Adapter: ::UINT, DeviceType: ::D3DDEVTYPE, hFocusWindow: ::HWND,
        BehaviorFlags: ::DWORD, pPresentationParameters: *mut ::D3DPRESENT_PARAMETERS,
        ppReturnedDeviceInterface: *mut *mut IDirect3DDevice9
    ) -> ::HRESULT
}
);
pub type LPDIRECT3D9 = *mut IDirect3D9;
pub type PDIRECT3D9 = *mut IDirect3D9;
RIDL!(
interface IDirect3DDevice9(IDirect3DDevice9Vtbl): IUnknown(IUnknownVtbl) {
    fn TestCooperativeLevel(&mut self) -> ::HRESULT,
    fn GetAvailableTextureMem(&mut self) -> ::UINT,
    fn EvictManagedResources(&mut self) -> ::HRESULT,
    fn GetDirect3D(&mut self, ppD3D9: *mut *mut IDirect3D9) -> ::HRESULT,
    fn GetDeviceCaps(&mut self, pCaps: *mut ::D3DCAPS9) -> ::HRESULT,
    fn GetDisplayMode(&mut self, iSwapChain: ::UINT, pMode: *mut ::D3DDISPLAYMODE) -> ::HRESULT,
    fn GetCreationParameters(
        &mut self, pParameters: *mut ::D3DDEVICE_CREATION_PARAMETERS
    ) -> ::HRESULT,
    fn SetCursorProperties(
        &mut self, XHotSpot: ::UINT, YHotSpot: ::UINT, pCursorBitmap: *mut IDirect3DSurface9
    ) -> ::HRESULT,
    fn SetCursorPosition(&mut self, X: ::INT, Y: ::INT, Flags: ::DWORD) -> (),
    fn ShowCursor(&mut self, bShow: ::BOOL) -> ::BOOL,
    fn CreateAdditionalSwapChain(
        &mut self, pPresentationParameters: *mut ::D3DPRESENT_PARAMETERS,
        pSwapChain: *mut *mut IDirect3DSwapChain9
    ) -> ::HRESULT,
    fn GetSwapChain(
        &mut self, iSwapChain: ::UINT, pSwapChain: *mut *mut IDirect3DSwapChain9
    ) -> ::HRESULT,
    fn GetNumberOfSwapChains(&mut self) -> ::UINT,
    fn Reset(&mut self, pPresentationParameters: *mut ::D3DPRESENT_PARAMETERS) -> ::HRESULT,
    fn Present(
        &mut self, pSourceRect: *const ::RECT, pDestRect: *const ::RECT,
        hDestWindowOverride: ::HWND, pDirtyRegion: *const ::RGNDATA
    ) -> ::HRESULT,
    fn GetBackBuffer(
        &mut self, iSwapChain: ::UINT, iBackBuffer: ::UINT, Type: ::D3DBACKBUFFER_TYPE,
        ppBackBuffer: *mut *mut IDirect3DSurface9
    ) -> ::HRESULT,
    fn GetRasterStatus(
        &mut self, iSwapChain: ::UINT, pRasterStatus: *mut ::D3DRASTER_STATUS
    ) -> ::HRESULT,
    fn SetDialogBoxMode(&mut self, bEnableDialogs: ::BOOL) -> ::HRESULT,
    fn SetGammaRamp(
        &mut self, iSwapChain: ::UINT, Flags: ::DWORD, pRamp: *const ::D3DGAMMARAMP
    ) -> (),
    fn GetGammaRamp(&mut self, iSwapChain: ::UINT, pRamp: *mut ::D3DGAMMARAMP) -> (),
    fn CreateTexture(
        &mut self, Width: ::UINT, Height: ::UINT, Levels: ::UINT, Usage: ::DWORD,
        Format: ::D3DFORMAT, Pool: ::D3DPOOL, ppTexture: *mut *mut IDirect3DTexture9,
        pSharedHandle: *mut ::HANDLE
    ) -> ::HRESULT,
    fn CreateVolumeTexture(
        &mut self, Width: ::UINT, Height: ::UINT, Depth: ::UINT, Levels: ::UINT, Usage: ::DWORD,
        Format: ::D3DFORMAT, Pool: ::D3DPOOL, ppVolumeTexture: *mut *mut IDirect3DVolumeTexture9,
        pSharedHandle: *mut ::HANDLE
    ) -> ::HRESULT,
    fn CreateCubeTexture(
        &mut self, EdgeLength: ::UINT, Levels: ::UINT, Usage: ::DWORD, Format: ::D3DFORMAT,
        Pool: ::D3DPOOL, ppCubeTexture: *mut *mut IDirect3DCubeTexture9,
        pSharedHandle: *mut ::HANDLE
    ) -> ::HRESULT,
    fn CreateVertexBuffer(
        &mut self, Length: ::UINT, Usage: ::DWORD, FVF: ::DWORD, Pool: ::D3DPOOL,
        ppVertexBuffer: *mut *mut IDirect3DVertexBuffer9, pSharedHandle: *mut ::HANDLE
    ) -> ::HRESULT,
    fn CreateIndexBuffer(
        &mut self, Length: ::UINT, Usage: ::DWORD, Format: ::D3DFORMAT, Pool: ::D3DPOOL,
        ppIndexBuffer: *mut *mut IDirect3DIndexBuffer9, pSharedHandle: *mut ::HANDLE
    ) -> ::HRESULT,
    fn CreateRenderTarget(
        &mut self, Width: ::UINT, Height: ::UINT, Format: ::D3DFORMAT,
        MultiSample: ::D3DMULTISAMPLE_TYPE, MultisampleQuality: ::DWORD, Lockable: ::BOOL,
        ppSurface: *mut *mut IDirect3DSurface9, pSharedHandle: *mut ::HANDLE
    ) -> ::HRESULT,
    fn CreateDepthStencilSurface(
        &mut self, Width: ::UINT, Height: ::UINT, Format: ::D3DFORMAT,
        MultiSample: ::D3DMULTISAMPLE_TYPE, MultisampleQuality: ::DWORD, Discard: ::BOOL,
        ppSurface: *mut *mut IDirect3DSurface9, pSharedHandle: *mut ::HANDLE
    ) -> ::HRESULT,
    fn UpdateSurface(
        &mut self, pSourceSurface: *mut IDirect3DSurface9, pSourceRect: *const ::RECT,
        pDestinationSurface: *mut IDirect3DSurface9, pDestPoint: *const ::POINT
    ) -> ::HRESULT,
    fn UpdateTexture(
        &mut self, pSourceTexture: *mut IDirect3DBaseTexture9,
        pDestinationTexture: *mut IDirect3DBaseTexture9
    ) -> ::HRESULT,
    fn GetRenderTargetData(
        &mut self, pRenderTarget: *mut IDirect3DSurface9, pDestSurface: *mut IDirect3DSurface9
    ) -> ::HRESULT,
    fn GetFrontBufferData(
        &mut self, iSwapChain: ::UINT, pDestSurface: *mut IDirect3DSurface9
    ) -> ::HRESULT,
    fn StretchRect(
        &mut self, pSourceSurface: *mut IDirect3DSurface9, pSourceRect: *const ::RECT,
        pDestSurface: *mut IDirect3DSurface9, pDestRect: *const ::RECT,
        Filter: ::D3DTEXTUREFILTERTYPE
    ) -> ::HRESULT,
    fn ColorFill(
        &mut self, pSurface: *mut IDirect3DSurface9, pRect: *const ::RECT, color: ::D3DCOLOR
    ) -> ::HRESULT,
    fn CreateOffscreenPlainSurface(
        &mut self, Width: ::UINT, Height: ::UINT, Format: ::D3DFORMAT, Pool: ::D3DPOOL,
        ppSurface: *mut *mut IDirect3DSurface9, pSharedHandle: *mut ::HANDLE
    ) -> ::HRESULT,
    fn SetRenderTarget(
        &mut self, RenderTargetIndex: ::DWORD, pRenderTarget: *mut IDirect3DSurface9
    ) -> ::HRESULT,
    fn GetRenderTarget(
        &mut self, RenderTargetIndex: ::DWORD, ppRenderTarget: *mut *mut IDirect3DSurface9
    ) -> ::HRESULT,
    fn SetDepthStencilSurface(&mut self, pNewZStencil: *mut IDirect3DSurface9) -> ::HRESULT,
    fn GetDepthStencilSurface(
        &mut self, ppZStencilSurface: *mut *mut IDirect3DSurface9
    ) -> ::HRESULT,
    fn BeginScene(&mut self) -> ::HRESULT,
    fn EndScene(&mut self) -> ::HRESULT,
    fn Clear(
        &mut self, Count: ::DWORD, pRects: *const ::D3DRECT, Flags: ::DWORD, Color: ::D3DCOLOR,
        Z: ::FLOAT, Stencil: ::DWORD
    ) -> ::HRESULT,
    fn SetTransform(
        &mut self, State: ::D3DTRANSFORMSTATETYPE, pMatrix: *const ::D3DMATRIX
    ) -> ::HRESULT,
    fn GetTransform(
        &mut self, State: ::D3DTRANSFORMSTATETYPE, pMatrix: *mut ::D3DMATRIX
    ) -> ::HRESULT,
    fn MultiplyTransform(
        &mut self, arg1: ::D3DTRANSFORMSTATETYPE, arg2: *const ::D3DMATRIX
    ) -> ::HRESULT,
    fn SetViewport(&mut self, pViewport: *const ::D3DVIEWPORT9) -> ::HRESULT,
    fn GetViewport(&mut self, pViewport: *mut ::D3DVIEWPORT9) -> ::HRESULT,
    fn SetMaterial(&mut self, pMaterial: *const ::D3DMATERIAL9) -> ::HRESULT,
    fn GetMaterial(&mut self, pMaterial: *mut ::D3DMATERIAL9) -> ::HRESULT,
    fn SetLight(&mut self, Index: ::DWORD, arg1: *const ::D3DLIGHT9) -> ::HRESULT,
    fn GetLight(&mut self, Index: ::DWORD, arg1: *mut ::D3DLIGHT9) -> ::HRESULT,
    fn LightEnable(&mut self, Index: ::DWORD, Enable: ::BOOL) -> ::HRESULT,
    fn GetLightEnable(&mut self, Index: ::DWORD, pEnable: *mut ::BOOL) -> ::HRESULT,
    fn SetClipPlane(&mut self, Index: ::DWORD, pPlane: *const ::FLOAT) -> ::HRESULT,
    fn GetClipPlane(&mut self, Index: ::DWORD, pPlane: *mut ::FLOAT) -> ::HRESULT,
    fn SetRenderState(&mut self, State: ::D3DRENDERSTATETYPE, Value: ::DWORD) -> ::HRESULT,
    fn GetRenderState(&mut self, State: ::D3DRENDERSTATETYPE, pValue: *mut ::DWORD) -> ::HRESULT,
    fn CreateStateBlock(
        &mut self, Type: ::D3DSTATEBLOCKTYPE, ppSB: *mut *mut IDirect3DStateBlock9
    ) -> ::HRESULT,
    fn BeginStateBlock(&mut self) -> ::HRESULT,
    fn EndStateBlock(&mut self, ppSB: *mut *mut IDirect3DStateBlock9) -> ::HRESULT,
    fn SetClipStatus(&mut self, pClipStatus: *const ::D3DCLIPSTATUS9) -> ::HRESULT,
    fn GetClipStatus(&mut self, pClipStatus: *mut ::D3DCLIPSTATUS9) -> ::HRESULT,
    fn GetTexture(
        &mut self, Stage: ::DWORD, ppTexture: *mut *mut IDirect3DBaseTexture9
    ) -> ::HRESULT,
    fn SetTexture(&mut self, Stage: ::DWORD, pTexture: *mut IDirect3DBaseTexture9) -> ::HRESULT,
    fn GetTextureStageState(
        &mut self, Stage: ::DWORD, Type: ::D3DTEXTURESTAGESTATETYPE, pValue: *mut ::DWORD
    ) -> ::HRESULT,
    fn SetTextureStageState(
        &mut self, Stage: ::DWORD, Type: ::D3DTEXTURESTAGESTATETYPE, Value: ::DWORD
    ) -> ::HRESULT,
    fn GetSamplerState(
        &mut self, Sampler: ::DWORD, Type: ::D3DSAMPLERSTATETYPE, pValue: *mut ::DWORD
    ) -> ::HRESULT,
    fn SetSamplerState(
        &mut self, Sampler: ::DWORD, Type: ::D3DSAMPLERSTATETYPE, Value: ::DWORD
    ) -> ::HRESULT,
    fn ValidateDevice(&mut self, pNumPasses: *mut ::DWORD) -> ::HRESULT,
    fn SetPaletteEntries(
        &mut self, PaletteNumber: ::UINT, pEntries: *const ::PALETTEENTRY
    ) -> ::HRESULT,
    fn GetPaletteEntries(
        &mut self, PaletteNumber: ::UINT, pEntries: *mut ::PALETTEENTRY
    ) -> ::HRESULT,
    fn SetCurrentTexturePalette(&mut self, PaletteNumber: ::UINT) -> ::HRESULT,
    fn GetCurrentTexturePalette(&mut self, PaletteNumber: *mut ::UINT) -> ::HRESULT,
    fn SetScissorRect(&mut self, pRect: *const ::RECT) -> ::HRESULT,
    fn GetScissorRect(&mut self, pRect: *mut ::RECT) -> ::HRESULT,
    fn SetSoftwareVertexProcessing(&mut self, bSoftware: ::BOOL) -> ::HRESULT,
    fn GetSoftwareVertexProcessing(&mut self) -> ::BOOL,
    fn SetNPatchMode(&mut self, nSegments: ::FLOAT) -> ::HRESULT,
    fn GetNPatchMode(&mut self) -> ::FLOAT,
    fn DrawPrimitive(
        &mut self, PrimitiveType: ::D3DPRIMITIVETYPE, StartVertex: ::UINT, PrimitiveCount: ::UINT
    ) -> ::HRESULT,
    fn DrawIndexedPrimitive(
        &mut self, arg1: ::D3DPRIMITIVETYPE, BaseVertexIndex: ::INT, MinVertexIndex: ::UINT,
        NumVertices: ::UINT, startIndex: ::UINT, primCount: ::UINT
    ) -> ::HRESULT,
    fn DrawPrimitiveUP(
        &mut self, PrimitiveType: ::D3DPRIMITIVETYPE, PrimitiveCount: ::UINT,
        pVertexStreamZeroData: *const ::VOID, VertexStreamZeroStride: ::UINT
    ) -> ::HRESULT,
    fn DrawIndexedPrimitiveUP(
        &mut self, PrimitiveType: ::D3DPRIMITIVETYPE, MinVertexIndex: ::UINT, NumVertices: ::UINT,
        PrimitiveCount: ::UINT, pIndexData: *const ::VOID, IndexDataFormat: ::D3DFORMAT,
        pVertexStreamZeroData: *const ::VOID, VertexStreamZeroStride: ::UINT
    ) -> ::HRESULT,
    fn ProcessVertices(
        &mut self, SrcStartIndex: ::UINT, DestIndex: ::UINT, VertexCount: ::UINT,
        pDestBuffer: *mut IDirect3DVertexBuffer9, pVertexDecl: *mut IDirect3DVertexDeclaration9,
        Flags: ::DWORD
    ) -> ::HRESULT,
    fn CreateVertexDeclaration(
        &mut self, pVertexElements: *const ::D3DVERTEXELEMENT9,
        ppDecl: *mut *mut IDirect3DVertexDeclaration9
    ) -> ::HRESULT,
    fn SetVertexDeclaration(&mut self, pDecl: *mut IDirect3DVertexDeclaration9) -> ::HRESULT,
    fn GetVertexDeclaration(&mut self, ppDecl: *mut *mut IDirect3DVertexDeclaration9) -> ::HRESULT,
    fn SetFVF(&mut self, FVF: ::DWORD) -> ::HRESULT,
    fn GetFVF(&mut self, pFVF: *mut ::DWORD) -> ::HRESULT,
    fn CreateVertexShader(
        &mut self, pFunction: *const ::DWORD, ppShader: *mut *mut IDirect3DVertexShader9
    ) -> ::HRESULT,
    fn SetVertexShader(&mut self, pShader: *mut IDirect3DVertexShader9) -> ::HRESULT,
    fn GetVertexShader(&mut self, ppShader: *mut *mut IDirect3DVertexShader9) -> ::HRESULT,
    fn SetVertexShaderConstantF(
        &mut self, StartRegister: ::UINT, pConstantData: *const ::FLOAT, Vector4fCount: ::UINT
    ) -> ::HRESULT,
    fn GetVertexShaderConstantF(
        &mut self, StartRegister: ::UINT, pConstantData: *mut ::FLOAT, Vector4fCount: ::UINT
    ) -> ::HRESULT,
    fn SetVertexShaderConstantI(
        &mut self, StartRegister: ::UINT, pConstantData: *const ::INT, Vector4iCount: ::UINT
    ) -> ::HRESULT,
    fn GetVertexShaderConstantI(
        &mut self, StartRegister: ::UINT, pConstantData: *mut ::INT, Vector4iCount: ::UINT
    ) -> ::HRESULT,
    fn SetVertexShaderConstantB(
        &mut self, StartRegister: ::UINT, pConstantData: *const ::BOOL, BoolCount: ::UINT
    ) -> ::HRESULT,
    fn GetVertexShaderConstantB(
        &mut self, StartRegister: ::UINT, pConstantData: *mut ::BOOL, BoolCount: ::UINT
    ) -> ::HRESULT,
    fn SetStreamSource(
        &mut self, StreamNumber: ::UINT, pStreamData: *mut IDirect3DVertexBuffer9,
        OffsetInBytes: ::UINT, Stride: ::UINT
    ) -> ::HRESULT,
    fn GetStreamSource(
        &mut self, StreamNumber: ::UINT, ppStreamData: *mut *mut IDirect3DVertexBuffer9,
        pOffsetInBytes: *mut ::UINT, pStride: *mut ::UINT
    ) -> ::HRESULT,
    fn SetStreamSourceFreq(&mut self, StreamNumber: ::UINT, Setting: ::UINT) -> ::HRESULT,
    fn GetStreamSourceFreq(&mut self, StreamNumber: ::UINT, pSetting: *mut ::UINT) -> ::HRESULT,
    fn SetIndices(&mut self, pIndexData: *mut IDirect3DIndexBuffer9) -> ::HRESULT,
    fn GetIndices(&mut self, ppIndexData: *mut *mut IDirect3DIndexBuffer9) -> ::HRESULT,
    fn CreatePixelShader(
        &mut self, pFunction: *const ::DWORD, ppShader: *mut *mut IDirect3DPixelShader9
    ) -> ::HRESULT,
    fn SetPixelShader(&mut self, pShader: *mut IDirect3DPixelShader9) -> ::HRESULT,
    fn GetPixelShader(&mut self, ppShader: *mut *mut IDirect3DPixelShader9) -> ::HRESULT,
    fn SetPixelShaderConstantF(
        &mut self, StartRegister: ::UINT, pConstantData: *const ::FLOAT, Vector4fCount: ::UINT
    ) -> ::HRESULT,
    fn GetPixelShaderConstantF(
        &mut self, StartRegister: ::UINT, pConstantData: *mut ::FLOAT, Vector4fCount: ::UINT
    ) -> ::HRESULT,
    fn SetPixelShaderConstantI(
        &mut self, StartRegister: ::UINT, pConstantData: *const ::INT, Vector4iCount: ::UINT
    ) -> ::HRESULT,
    fn GetPixelShaderConstantI(
        &mut self, StartRegister: ::UINT, pConstantData: *mut ::INT, Vector4iCount: ::UINT
    ) -> ::HRESULT,
    fn SetPixelShaderConstantB(
        &mut self, StartRegister: ::UINT, pConstantData: *const ::BOOL, BoolCount: ::UINT
    ) -> ::HRESULT,
    fn GetPixelShaderConstantB(
        &mut self, StartRegister: ::UINT, pConstantData: *mut ::BOOL, BoolCount: ::UINT
    ) -> ::HRESULT,
    fn DrawRectPatch(
        &mut self, Handle: ::UINT, pNumSegs: *const ::FLOAT,
        pRectPatchInfo: *const ::D3DRECTPATCH_INFO
    ) -> ::HRESULT,
    fn DrawTriPatch(
        &mut self, Handle: ::UINT, pNumSegs: *const ::FLOAT,
        pTriPatchInfo: *const ::D3DTRIPATCH_INFO
    ) -> ::HRESULT,
    fn DeletePatch(&mut self, Handle: ::UINT) -> ::HRESULT,
    fn CreateQuery(
        &mut self, Type: ::D3DQUERYTYPE, ppQuery: *mut *mut IDirect3DQuery9
    ) -> ::HRESULT
}
);
pub type LPDIRECT3DDEVICE9 = *mut IDirect3DDevice9;
pub type PDIRECT3DDEVICE9 = *mut IDirect3DDevice9;
RIDL!(
interface IDirect3DStateBlock9(IDirect3DStateBlock9Vtbl): IUnknown(IUnknownVtbl) {
    fn GetDevice(&mut self, ppDevice: *mut *mut IDirect3DDevice9) -> ::HRESULT,
    fn Capture(&mut self) -> ::HRESULT,
    fn Apply(&mut self) -> ::HRESULT
}
);
pub type LPDIRECT3DSTATEBLOCK9 = *mut IDirect3DStateBlock9;
pub type PDIRECT3DSTATEBLOCK9 = *mut IDirect3DStateBlock9;
RIDL!(
interface IDirect3DSwapChain9(IDirect3DSwapChain9Vtbl): IUnknown(IUnknownVtbl) {
    fn Present(
        &mut self, pSourceRect: *const ::RECT, pDestRect: *const ::RECT,
        hDestWindowOverride: ::HWND, pDirtyRegion: *const ::RGNDATA, dwFlags: ::DWORD
    ) -> ::HRESULT,
    fn GetFrontBufferData(&mut self, pDestSurface: *mut IDirect3DSurface9) -> ::HRESULT,
    fn GetBackBuffer(
        &mut self, iBackBuffer: ::UINT, Type: ::D3DBACKBUFFER_TYPE,
        ppBackBuffer: *mut *mut IDirect3DSurface9
    ) -> ::HRESULT,
    fn GetRasterStatus(&mut self, pRasterStatus: *mut ::D3DRASTER_STATUS) -> ::HRESULT,
    fn GetDisplayMode(&mut self, pMode: *mut ::D3DDISPLAYMODE) -> ::HRESULT,
    fn GetDevice(&mut self, ppDevice: *mut *mut IDirect3DDevice9) -> ::HRESULT,
    fn GetPresentParameters(
        &mut self, pPresentationParameters: *mut ::D3DPRESENT_PARAMETERS
    ) -> ::HRESULT
}
);
pub type LPDIRECT3DSWAPCHAIN9 = *mut IDirect3DSwapChain9;
pub type PDIRECT3DSWAPCHAIN9 = *mut IDirect3DSwapChain9;
RIDL!(
interface IDirect3DResource9(IDirect3DResource9Vtbl): IUnknown(IUnknownVtbl) {
    fn GetDevice(&mut self, ppDevice: *mut *mut IDirect3DDevice9) -> ::HRESULT,
    fn SetPrivateData(
        &mut self, refguid: *const ::GUID, pData: *const ::VOID, SizeOfData: ::DWORD,
        Flags: ::DWORD
    ) -> ::HRESULT,
    fn GetPrivateData(
        &mut self, refguid: *const ::GUID, pData: *mut ::VOID, pSizeOfData: *mut ::DWORD
    ) -> ::HRESULT,
    fn FreePrivateData(&mut self, refguid: *const ::GUID) -> ::HRESULT,
    fn SetPriority(&mut self, PriorityNew: ::DWORD) -> ::DWORD,
    fn GetPriority(&mut self) -> ::DWORD,
    fn PreLoad(&mut self) -> (),
    fn GetType(&mut self) -> ::D3DRESOURCETYPE
}
);
pub type LPDIRECT3DRESOURCE9 = *mut IDirect3DResource9;
pub type PDIRECT3DRESOURCE9 = *mut IDirect3DResource9;
RIDL!(
interface IDirect3DVertexDeclaration9(IDirect3DVertexDeclaration9Vtbl): IUnknown(IUnknownVtbl) {
    fn GetDevice(&mut self, ppDevice: *mut *mut IDirect3DDevice9) -> ::HRESULT,
    fn GetDeclaration(
        &mut self, pElement: *mut ::D3DVERTEXELEMENT9, pNumElements: *mut ::UINT
    ) -> ::HRESULT
}
);
pub type LPDIRECT3DVERTEXDECLARATION9 = *mut IDirect3DVertexDeclaration9;
pub type PDIRECT3DVERTEXDECLARATION9 = *mut IDirect3DVertexDeclaration9;
RIDL!(
interface IDirect3DVertexShader9(IDirect3DVertexShader9Vtbl): IUnknown(IUnknownVtbl) {
    fn GetDevice(&mut self, ppDevice: *mut *mut IDirect3DDevice9) -> ::HRESULT,
    fn GetFunction(&mut self, arg1: *mut ::VOID, pSizeOfData: *mut ::UINT) -> ::HRESULT
}
);
pub type LPDIRECT3DVERTEXSHADER9 = *mut IDirect3DVertexShader9;
pub type PDIRECT3DVERTEXSHADER9 = *mut IDirect3DVertexShader9;
RIDL!(
interface IDirect3DPixelShader9(IDirect3DPixelShader9Vtbl): IUnknown(IUnknownVtbl) {
    fn GetDevice(&mut self, ppDevice: *mut *mut IDirect3DDevice9) -> ::HRESULT,
    fn GetFunction(&mut self, arg1: *mut ::VOID, pSizeOfData: *mut ::UINT) -> ::HRESULT
}
);
pub type LPDIRECT3DPIXELSHADER9 = *mut IDirect3DPixelShader9;
pub type PDIRECT3DPIXELSHADER9 = *mut IDirect3DPixelShader9;
RIDL!(
interface IDirect3DBaseTexture9(IDirect3DBaseTexture9Vtbl): IDirect3DResource9(IDirect3DResource9Vtbl) {
    fn SetLOD(&mut self, LODNew: ::DWORD) -> ::DWORD,
    fn GetLOD(&mut self) -> ::DWORD,
    fn GetLevelCount(&mut self) -> ::DWORD,
    fn SetAutoGenFilterType(&mut self, FilterType: ::D3DTEXTUREFILTERTYPE) -> ::HRESULT,
    fn GetAutoGenFilterType(&mut self) -> ::D3DTEXTUREFILTERTYPE,
    fn GenerateMipSubLevels(&mut self) -> ()
}
);
pub type LPDIRECT3DBASETEXTURE9 = *mut IDirect3DBaseTexture9;
pub type PDIRECT3DBASETEXTURE9 = *mut IDirect3DBaseTexture9;
RIDL!(
interface IDirect3DTexture9(IDirect3DTexture9Vtbl): IDirect3DBaseTexture9(IDirect3DBaseTexture9Vtbl) {
    fn GetLevelDesc(&mut self, Level: ::UINT, pDesc: *mut ::D3DSURFACE_DESC) -> ::HRESULT,
    fn GetSurfaceLevel(
        &mut self, Level: ::UINT, ppSurfaceLevel: *mut *mut IDirect3DSurface9
    ) -> ::HRESULT,
    fn LockRect(
        &mut self, Level: ::UINT, pLockedRect: *mut ::D3DLOCKED_RECT, pRect: *const ::RECT,
        Flags: ::DWORD
    ) -> ::HRESULT,
    fn UnlockRect(&mut self, Level: ::UINT) -> ::HRESULT,
    fn AddDirtyRect(&mut self, pDirtyRect: *const ::RECT) -> ::HRESULT
}
);
pub type LPDIRECT3DTEXTURE9 = *mut IDirect3DTexture9;
pub type PDIRECT3DTEXTURE9 = *mut IDirect3DTexture9;
RIDL!(
interface IDirect3DVolumeTexture9(IDirect3DVolumeTexture9Vtbl): IDirect3DBaseTexture9(IDirect3DBaseTexture9Vtbl) {
    fn GetLevelDesc(&mut self, Level: ::UINT, pDesc: *mut ::D3DVOLUME_DESC) -> ::HRESULT,
    fn GetVolumeLevel(
        &mut self, Level: ::UINT, ppVolumeLevel: *mut *mut IDirect3DVolume9
    ) -> ::HRESULT,
    fn LockBox(
        &mut self, Level: ::UINT, pLockedVolume: *mut ::D3DLOCKED_BOX, pBox: *const ::D3DBOX,
        Flags: ::DWORD
    ) -> ::HRESULT,
    fn UnlockBox(&mut self, Level: ::UINT) -> ::HRESULT,
    fn AddDirtyBox(&mut self, pDirtyBox: *const ::D3DBOX) -> ::HRESULT
}
);
pub type LPDIRECT3DVOLUMETEXTURE9 = *mut IDirect3DVolumeTexture9;
pub type PDIRECT3DVOLUMETEXTURE9 = *mut IDirect3DVolumeTexture9;
RIDL!(
interface IDirect3DCubeTexture9(IDirect3DCubeTexture9Vtbl): IDirect3DBaseTexture9(IDirect3DBaseTexture9Vtbl) {
    fn GetLevelDesc(&mut self, Level: ::UINT, pDesc: *mut ::D3DSURFACE_DESC) -> ::HRESULT,
    fn GetCubeMapSurface(
        &mut self, FaceType: ::D3DCUBEMAP_FACES, Level: ::UINT,
        ppCubeMapSurface: *mut *mut IDirect3DSurface9
    ) -> ::HRESULT,
    fn LockRect(
        &mut self, FaceType: ::D3DCUBEMAP_FACES, Level: ::UINT, pLockedRect: *mut ::D3DLOCKED_RECT,
        pRect: *const ::RECT, Flags: ::DWORD
    ) -> ::HRESULT,
    fn UnlockRect(&mut self, FaceType: ::D3DCUBEMAP_FACES, Level: ::UINT) -> ::HRESULT,
    fn AddDirtyRect(
        &mut self, FaceType: ::D3DCUBEMAP_FACES, pDirtyRect: *const ::RECT
    ) -> ::HRESULT
}
);
pub type LPDIRECT3DCUBETEXTURE9 = *mut IDirect3DCubeTexture9;
pub type PDIRECT3DCUBETEXTURE9 = *mut IDirect3DCubeTexture9;
RIDL!(
interface IDirect3DVertexBuffer9(IDirect3DVertexBuffer9Vtbl): IDirect3DResource9(IDirect3DResource9Vtbl) {
    fn Lock(
        &mut self, OffsetToLock: ::UINT, SizeToLock: ::UINT, ppbData: *mut *mut ::VOID,
        Flags: ::DWORD
    ) -> ::HRESULT,
    fn Unlock(&mut self) -> ::HRESULT,
    fn GetDesc(&mut self, pDesc: *mut ::D3DVERTEXBUFFER_DESC) -> ::HRESULT
}
);
pub type LPDIRECT3DVERTEXBUFFER9 = *mut IDirect3DVertexBuffer9;
pub type PDIRECT3DVERTEXBUFFER9 = *mut IDirect3DVertexBuffer9;
RIDL!(
interface IDirect3DIndexBuffer9(IDirect3DIndexBuffer9Vtbl): IDirect3DResource9(IDirect3DResource9Vtbl) {
    fn Lock(
        &mut self, OffsetToLock: ::UINT, SizeToLock: ::UINT, ppbData: *mut *mut ::VOID,
        Flags: ::DWORD
    ) -> ::HRESULT,
    fn Unlock(&mut self) -> ::HRESULT,
    fn GetDesc(&mut self, pDesc: *mut ::D3DINDEXBUFFER_DESC) -> ::HRESULT
}
);
pub type LPDIRECT3DINDEXBUFFER9 = *mut IDirect3DIndexBuffer9;
pub type PDIRECT3DINDEXBUFFER9 = *mut IDirect3DIndexBuffer9;
RIDL!(
interface IDirect3DSurface9(IDirect3DSurface9Vtbl): IDirect3DResource9(IDirect3DResource9Vtbl) {
    fn GetContainer(&mut self, riid: *const ::IID, ppContainer: *mut *mut ::VOID) -> ::HRESULT,
    fn GetDesc(&mut self, pDesc: *mut ::D3DSURFACE_DESC) -> ::HRESULT,
    fn LockRect(
        &mut self, pLockedRect: *mut ::D3DLOCKED_RECT, pRect: *const ::RECT, Flags: ::DWORD
    ) -> ::HRESULT,
    fn UnlockRect(&mut self) -> ::HRESULT,
    fn GetDC(&mut self, phdc: *mut ::HDC) -> ::HRESULT,
    fn ReleaseDC(&mut self, hdc: ::HDC) -> ::HRESULT
}
);
pub type LPDIRECT3DSURFACE9 = *mut IDirect3DSurface9;
pub type PDIRECT3DSURFACE9 = *mut IDirect3DSurface9;
RIDL!(
interface IDirect3DVolume9(IDirect3DVolume9Vtbl): IUnknown(IUnknownVtbl) {
    fn GetDevice(&mut self, ppDevice: *mut *mut IDirect3DDevice9) -> ::HRESULT,
    fn SetPrivateData(
        &mut self, refguid: *const ::GUID, pData: *const ::VOID, SizeOfData: ::DWORD,
        Flags: ::DWORD
    ) -> ::HRESULT,
    fn GetPrivateData(
        &mut self, refguid: *const ::GUID, pData: *mut ::VOID, pSizeOfData: *mut ::DWORD
    ) -> ::HRESULT,
    fn FreePrivateData(&mut self, refguid: *const ::GUID) -> ::HRESULT,
    fn GetContainer(&mut self, riid: *const ::IID, ppContainer: *mut *mut ::VOID) -> ::HRESULT,
    fn GetDesc(&mut self, pDesc: *mut ::D3DVOLUME_DESC) -> ::HRESULT,
    fn LockBox(
        &mut self, pLockedVolume: *mut ::D3DLOCKED_BOX, pBox: *const ::D3DBOX, Flags: ::DWORD
    ) -> ::HRESULT,
    fn UnlockBox(&mut self) -> ::HRESULT
}
);
pub type LPDIRECT3DVOLUME9 = *mut IDirect3DVolume9;
pub type PDIRECT3DVOLUME9 = *mut IDirect3DVolume9;
RIDL!(
interface IDirect3DQuery9(IDirect3DQuery9Vtbl): IUnknown(IUnknownVtbl) {
    fn GetDevice(&mut self, ppDevice: *mut *mut IDirect3DDevice9) -> ::HRESULT,
    fn GetType(&mut self) -> ::D3DRESOURCETYPE,
    fn GetDataSize(&mut self) -> ::DWORD,
    fn Issue(&mut self, dwIssueFlags: ::DWORD) -> ::HRESULT,
    fn GetData(
        &mut self, pData: *mut ::VOID, dwSize: ::DWORD, dwGetDataFlags: ::DWORD
    ) -> ::HRESULT
}
);
pub type LPDIRECT3DQUERY9 = *mut IDirect3DQuery9;
pub type PDIRECT3DQUERY9 = *mut IDirect3DQuery9;
pub const D3DCREATE_FPU_PRESERVE: ::DWORD = 0x2;
pub const D3DCREATE_MULTITHREADED: ::DWORD = 0x4;
pub const D3DCREATE_PUREDEVICE: ::DWORD = 0x10;
pub const D3DCREATE_SOFTWARE_VERTEXPROCESSING: ::DWORD = 0x20;
pub const D3DCREATE_HARDWARE_VERTEXPROCESSING: ::DWORD = 0x40;
pub const D3DCREATE_MIXED_VERTEXPROCESSING: ::DWORD = 0x80;
pub const D3DCREATE_DISABLE_DRIVER_MANAGEMENT: ::DWORD = 0x100;
pub const D3DCREATE_ADAPTERGROUP_DEVICE: ::DWORD = 0x200;
pub const D3DCREATE_DISABLE_DRIVER_MANAGEMENT_EX: ::DWORD = 0x400;
pub const D3DCREATE_NOWINDOWCHANGES: ::DWORD = 0x800;
pub const D3DCREATE_DISABLE_PSGP_THREADING: ::DWORD = 0x2000;
pub const D3DCREATE_ENABLE_PRESENTSTATS: ::DWORD = 0x4000;
pub const D3DCREATE_DISABLE_PRESENTSTATS: ::DWORD = 0x8000;
pub const D3DCREATE_SCREENSAVER: ::DWORD = 0x10000000;
pub const D3DADAPTER_DEFAULT: ::DWORD = 0;
RIDL!(
interface IDirect3D9Ex(IDirect3D9ExVtbl): IDirect3D9(IDirect3D9Vtbl) {
    fn GetAdapterModeCountEx(
        &mut self, Adapter: ::UINT, pFilter: *const ::D3DDISPLAYMODEFILTER
    ) -> ::UINT,
    fn EnumAdapterModesEx(
        &mut self, Adapter: ::UINT, pFilter: *const ::D3DDISPLAYMODEFILTER, Mode: ::UINT,
        pMode: *mut ::D3DDISPLAYMODEEX
    ) -> ::HRESULT,
    fn GetAdapterDisplayModeEx(
        &mut self, Adapter: ::UINT, pMode: *mut ::D3DDISPLAYMODEEX,
        pRotation: *mut ::D3DDISPLAYROTATION
    ) -> ::HRESULT,
    fn CreateDeviceEx(
        &mut self, Adapter: ::UINT, DeviceType: ::D3DDEVTYPE, hFocusWindow: ::HWND,
        BehaviorFlags: ::DWORD, pPresentationParameters: *mut ::D3DPRESENT_PARAMETERS,
        pFullscreenDisplayMode: *mut ::D3DDISPLAYMODEEX,
        ppReturnedDeviceInterface: *mut *mut IDirect3DDevice9Ex
    ) -> ::HRESULT,
    fn GetAdapterLUID(&mut self, Adapter: ::UINT, pLUID: *mut ::LUID) -> ::HRESULT
}
);
pub type LPDIRECT3D9EX = *mut IDirect3D9Ex;
pub type PDIRECT3D9EX = *mut IDirect3D9Ex;
RIDL!(
interface IDirect3DDevice9Ex(IDirect3DDevice9ExVtbl): IDirect3DDevice9(IDirect3DDevice9Vtbl) {
    fn SetConvolutionMonoKernel(
        &mut self, width: ::UINT, height: ::UINT, rows: *mut ::FLOAT, columns: *mut ::FLOAT
    ) -> ::HRESULT,
    fn ComposeRects(
        &mut self, pSrc: *mut IDirect3DSurface9, pDst: *mut IDirect3DSurface9,
        pSrcRectDescs: *mut IDirect3DVertexBuffer9, NumRects: ::UINT,
        pDstRectDescs: *mut IDirect3DVertexBuffer9, Operation: ::D3DCOMPOSERECTSOP, Xoffset: ::INT,
        Yoffset: ::INT
    ) -> ::HRESULT,
    fn PresentEx(
        &mut self, pSourceRect: *const ::RECT, pDestRect: *const ::RECT,
        hDestWindowOverride: ::HWND, pDirtyRegion: *const ::RGNDATA, dwFlags: ::DWORD
    ) -> ::HRESULT,
    fn GetGPUThreadPriority(&mut self, pPriority: *mut ::INT) -> ::HRESULT,
    fn SetGPUThreadPriority(&mut self, Priority: ::INT) -> ::HRESULT,
    fn WaitForVBlank(&mut self, iSwapChain: ::UINT) -> ::HRESULT,
    fn CheckResourceResidency(
        &mut self, pResourceArray: *mut *mut IDirect3DResource9, NumResources: ::UINT32
    ) -> ::HRESULT,
    fn SetMaximumFrameLatency(&mut self, MaxLatency: ::UINT) -> ::HRESULT,
    fn GetMaximumFrameLatency(&mut self, pMaxLatency: *mut ::UINT) -> ::HRESULT,
    fn CheckDeviceState(&mut self, hDestinationWindow: ::HWND) -> ::HRESULT,
    fn CreateRenderTargetEx(
        &mut self, Width: ::UINT, Height: ::UINT, Format: ::D3DFORMAT,
        MultiSample: ::D3DMULTISAMPLE_TYPE, MultisampleQuality: ::DWORD, Lockable: ::BOOL,
        ppSurface: *mut *mut IDirect3DSurface9, pSharedHandle: *mut ::HANDLE, Usage: ::DWORD
    ) -> ::HRESULT,
    fn CreateOffscreenPlainSurfaceEx(
        &mut self, Width: ::UINT, Height: ::UINT, Format: ::D3DFORMAT, Pool: ::D3DPOOL,
        ppSurface: *mut *mut IDirect3DSurface9, pSharedHandle: *mut ::HANDLE, Usage: ::DWORD
    ) -> ::HRESULT,
    fn CreateDepthStencilSurfaceEx(
        &mut self, Width: ::UINT, Height: ::UINT, Format: ::D3DFORMAT,
        MultiSample: ::D3DMULTISAMPLE_TYPE, MultisampleQuality: ::DWORD, Discard: ::BOOL,
        ppSurface: *mut *mut IDirect3DSurface9, pSharedHandle: *mut ::HANDLE, Usage: ::DWORD
    ) -> ::HRESULT,
    fn ResetEx(
        &mut self, pPresentationParameters: *mut ::D3DPRESENT_PARAMETERS,
        pFullscreenDisplayMode: *mut ::D3DDISPLAYMODEEX
    ) -> ::HRESULT,
    fn GetDisplayModeEx(
        &mut self, iSwapChain: ::UINT, pMode: *mut ::D3DDISPLAYMODEEX,
        pRotation: *mut ::D3DDISPLAYROTATION
    ) -> ::HRESULT
}
);
pub type LPDIRECT3DDEVICE9EX = *mut IDirect3DDevice9Ex;
pub type PDIRECT3DDEVICE9EX = *mut IDirect3DDevice9Ex;
RIDL!(
interface IDirect3DSwapChain9Ex(IDirect3DSwapChain9ExVtbl): IDirect3DSwapChain9(IDirect3DSwapChain9Vtbl) {
    fn GetLastPresentCount(&mut self, pLastPresentCount: *mut ::UINT) -> ::HRESULT,
    fn GetPresentStats(&mut self, pPresentationStatistics: *mut ::D3DPRESENTSTATS) -> ::HRESULT,
    fn GetDisplayModeEx(
        &mut self, pMode: *mut ::D3DDISPLAYMODEEX, pRotation: *mut ::D3DDISPLAYROTATION
    ) -> ::HRESULT
}
);
pub type LPDIRECT3DSWAPCHAIN9EX = *mut IDirect3DSwapChain9Ex;
pub type PDIRECT3DSWAPCHAIN9EX = *mut IDirect3DSwapChain9Ex;
RIDL!(
interface IDirect3D9ExOverlayExtension(IDirect3D9ExOverlayExtensionVtbl): IUnknown(IUnknownVtbl) {
    fn CheckDeviceOverlayType(
        &mut self, Adapter: ::UINT, DevType: ::D3DDEVTYPE, OverlayWidth: ::UINT,
        OverlayHeight: ::UINT, OverlayFormat: ::D3DFORMAT, pDisplayMode: *mut ::D3DDISPLAYMODEEX,
        DisplayRotation: ::D3DDISPLAYROTATION, pOverlayCaps: *mut ::D3DOVERLAYCAPS
    ) -> ::HRESULT
}
);
pub type LPDIRECT3D9EXOVERLAYEXTENSION = *mut IDirect3D9ExOverlayExtension;
pub type PDIRECT3D9EXOVERLAYEXTENSION = *mut IDirect3D9ExOverlayExtension;
RIDL!(
interface IDirect3DDevice9Video(IDirect3DDevice9VideoVtbl): IUnknown(IUnknownVtbl) {
    fn GetContentProtectionCaps(
        &mut self, pCryptoType: *const ::GUID, pDecodeProfile: *const ::GUID,
        pCaps: *mut ::D3DCONTENTPROTECTIONCAPS
    ) -> ::HRESULT,
    fn CreateAuthenticatedChannel(
        &mut self, ChannelType: ::D3DAUTHENTICATEDCHANNELTYPE,
        ppAuthenticatedChannel: *mut *mut IDirect3DAuthenticatedChannel9,
        pChannelHandle: *mut ::HANDLE
    ) -> ::HRESULT,
    fn CreateCryptoSession(
        &mut self, pCryptoType: *const ::GUID, pDecodeProfile: *const ::GUID,
        ppCryptoSession: *mut *mut IDirect3DCryptoSession9, pCryptoHandle: *mut ::HANDLE
    ) -> ::HRESULT
}
);
pub type LPDIRECT3DDEVICE9VIDEO = *mut IDirect3DDevice9Video;
pub type PDIRECT3DDEVICE9VIDEO = *mut IDirect3DDevice9Video;
RIDL!(
interface IDirect3DAuthenticatedChannel9(IDirect3DAuthenticatedChannel9Vtbl): IUnknown(IUnknownVtbl) {
    fn GetCertificateSize(&mut self, pCertificateSize: *mut ::UINT) -> ::HRESULT,
    fn GetCertificate(&mut self, CertifacteSize: ::UINT, ppCertificate: *mut ::BYTE) -> ::HRESULT,
    fn NegotiateKeyExchange(&mut self, DataSize: ::UINT, pData: *mut ::VOID) -> ::HRESULT,
    fn Query(
        &mut self, InputSize: ::UINT, pInput: *const ::VOID, OutputSize: ::UINT,
        pOutput: *mut ::VOID
    ) -> ::HRESULT,
    fn Configure(
        &mut self, InputSize: ::UINT, pInput: *const ::VOID,
        pOutput: *mut ::D3DAUTHENTICATEDCHANNEL_CONFIGURE_OUTPUT
    ) -> ::HRESULT
}
);
pub type LPDIRECT3DAUTHENTICATEDCHANNEL9 = *mut IDirect3DAuthenticatedChannel9;
pub type PDIRECT3DAUTHENTICATEDCHANNEL9 = *mut IDirect3DAuthenticatedChannel9;
RIDL!(
interface IDirect3DCryptoSession9(IDirect3DCryptoSession9Vtbl): IUnknown(IUnknownVtbl) {
    fn GetCertificateSize(&mut self, pCertificateSize: *mut ::UINT) -> ::HRESULT,
    fn GetCertificate(&mut self, CertifacteSize: ::UINT, ppCertificate: *mut ::BYTE) -> ::HRESULT,
    fn NegotiateKeyExchange(&mut self, DataSize: ::UINT, pData: *mut ::VOID) -> ::HRESULT,
    fn EncryptionBlt(
        &mut self, pSrcSurface: *mut IDirect3DSurface9, pDstSurface: *mut IDirect3DSurface9,
        DstSurfaceSize: ::UINT, pIV: *mut ::VOID
    ) -> ::HRESULT,
    fn DecryptionBlt(
        &mut self, pSrcSurface: *mut IDirect3DSurface9, pDstSurface: *mut IDirect3DSurface9,
        SrcSurfaceSize: ::UINT, pEncryptedBlockInfo: *mut ::D3DENCRYPTED_BLOCK_INFO,
        pContentKey: *mut ::VOID, pIV: *mut ::VOID
    ) -> ::HRESULT,
    fn GetSurfacePitch(
        &mut self, pSrcSurface: *mut IDirect3DSurface9, pSurfacePitch: *mut ::UINT
    ) -> ::HRESULT,
    fn StartSessionKeyRefresh(
        &mut self, pRandomNumber: *mut ::VOID, RandomNumberSize: ::UINT
    ) -> ::HRESULT,
    fn FinishSessionKeyRefresh(&mut self) -> ::HRESULT,
    fn GetEncryptionBltKey(&mut self, pReadbackKey: *mut ::VOID, KeySize: ::UINT) -> ::HRESULT
}
);
pub type LPDIRECT3DCRYPTOSESSION9 = *mut IDirect3DCryptoSession9;
pub type PDIRECT3DCRYPTOSESSION9 = *mut IDirect3DCryptoSession9;

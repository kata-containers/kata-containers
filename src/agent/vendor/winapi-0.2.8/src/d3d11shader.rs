// Copyright © 2016, Peter Atashian
// Licensed under the MIT License <LICENSE.md>
use super::*;
ENUM!{enum D3D11_SHADER_VERSION_TYPE {
    D3D11_SHVER_PIXEL_SHADER = 0,
    D3D11_SHVER_VERTEX_SHADER = 1,
    D3D11_SHVER_GEOMETRY_SHADER = 2,
    D3D11_SHVER_HULL_SHADER = 3,
    D3D11_SHVER_DOMAIN_SHADER = 4,
    D3D11_SHVER_COMPUTE_SHADER = 5,
    D3D11_SHVER_RESERVED0 = 0xFFF0,
}}
pub const D3D_RETURN_PARAMETER_INDEX: c_int = -1;
pub type D3D11_RESOURCE_RETURN_TYPE = D3D_RESOURCE_RETURN_TYPE;
pub type D3D11_CBUFFER_TYPE = D3D_CBUFFER_TYPE;
STRUCT!{struct D3D11_SIGNATURE_PARAMETER_DESC {
    SemanticName: LPCSTR,
    SemanticIndex: UINT,
    Register: UINT,
    SystemValueType: D3D_NAME,
    ComponentType: D3D_REGISTER_COMPONENT_TYPE,
    Mask: BYTE,
    ReadWriteMask: BYTE,
    Stream: UINT,
    MinPrecision: D3D_MIN_PRECISION,
}}
STRUCT!{struct D3D11_SHADER_BUFFER_DESC {
    Name: LPCSTR,
    Type: D3D_CBUFFER_TYPE,
    Variables: UINT,
    Size: UINT,
    uFlags: UINT,
}}
STRUCT!{struct D3D11_SHADER_VARIABLE_DESC {
    Name: LPCSTR,
    StartOffset: UINT,
    Size: UINT,
    uFlags: UINT,
    DefaultValue: LPVOID,
    StartTexture: UINT,
    TextureSize: UINT,
    StartSampler: UINT,
    SamplerSize: UINT,
}}
STRUCT!{struct D3D11_SHADER_TYPE_DESC {
    Class: D3D_SHADER_VARIABLE_CLASS,
    Type: D3D_SHADER_VARIABLE_TYPE,
    Rows: UINT,
    Columns: UINT,
    Elements: UINT,
    Members: UINT,
    Offset: UINT,
    Name: LPCSTR,
}}
pub type D3D11_TESSELLATOR_DOMAIN = D3D_TESSELLATOR_DOMAIN;
pub type D3D11_TESSELLATOR_PARTITIONING = D3D_TESSELLATOR_PARTITIONING;
pub type D3D11_TESSELLATOR_OUTPUT_PRIMITIVE = D3D_TESSELLATOR_OUTPUT_PRIMITIVE;
STRUCT!{struct D3D11_SHADER_DESC {
    Version: UINT,
    Creator: LPCSTR,
    Flags: UINT,
    ConstantBuffers: UINT,
    BoundResources: UINT,
    InputParameters: UINT,
    OutputParameters: UINT,
    InstructionCount: UINT,
    TempRegisterCount: UINT,
    TempArrayCount: UINT,
    DefCount: UINT,
    DclCount: UINT,
    TextureNormalInstructions: UINT,
    TextureLoadInstructions: UINT,
    TextureCompInstructions: UINT,
    TextureBiasInstructions: UINT,
    TextureGradientInstructions: UINT,
    FloatInstructionCount: UINT,
    IntInstructionCount: UINT,
    UintInstructionCount: UINT,
    StaticFlowControlCount: UINT,
    DynamicFlowControlCount: UINT,
    MacroInstructionCount: UINT,
    ArrayInstructionCount: UINT,
    CutInstructionCount: UINT,
    EmitInstructionCount: UINT,
    GSOutputTopology: D3D_PRIMITIVE_TOPOLOGY,
    GSMaxOutputVertexCount: UINT,
    InputPrimitive: D3D_PRIMITIVE,
    PatchConstantParameters: UINT,
    cGSInstanceCount: UINT,
    cControlPoints: UINT,
    HSOutputPrimitive: D3D_TESSELLATOR_OUTPUT_PRIMITIVE,
    HSPartitioning: D3D_TESSELLATOR_PARTITIONING,
    TessellatorDomain: D3D_TESSELLATOR_DOMAIN,
    cBarrierInstructions: UINT,
    cInterlockedInstructions: UINT,
    cTextureStoreInstructions: UINT,
}}
STRUCT!{struct D3D11_SHADER_INPUT_BIND_DESC {
    Name: LPCSTR,
    Type: D3D_SHADER_INPUT_TYPE,
    BindPoint: UINT,
    BindCount: UINT,
    uFlags: UINT,
    ReturnType: D3D_RESOURCE_RETURN_TYPE,
    Dimension: D3D_SRV_DIMENSION,
    NumSamples: UINT,
}}
pub const D3D_SHADER_REQUIRES_DOUBLES: UINT64 = 0x00000001;
pub const D3D_SHADER_REQUIRES_EARLY_DEPTH_STENCIL: UINT64 = 0x00000002;
pub const D3D_SHADER_REQUIRES_UAVS_AT_EVERY_STAGE: UINT64 = 0x00000004;
pub const D3D_SHADER_REQUIRES_64_UAVS: UINT64 = 0x00000008;
pub const D3D_SHADER_REQUIRES_MINIMUM_PRECISION: UINT64 = 0x00000010;
pub const D3D_SHADER_REQUIRES_11_1_DOUBLE_EXTENSIONS: UINT64 = 0x00000020;
pub const D3D_SHADER_REQUIRES_11_1_SHADER_EXTENSIONS: UINT64 = 0x00000040;
pub const D3D_SHADER_REQUIRES_LEVEL_9_COMPARISON_FILTERING: UINT64 = 0x00000080;
pub const D3D_SHADER_REQUIRES_TILED_RESOURCES: UINT64 = 0x00000100;
STRUCT!{struct D3D11_LIBRARY_DESC {
    Creator: LPCSTR,
    Flags: UINT,
    FunctionCount: UINT,
}}
STRUCT!{struct D3D11_FUNCTION_DESC {
    Version: UINT,
    Creator: LPCSTR,
    Flags: UINT,
    ConstantBuffers: UINT,
    BoundResources: UINT,
    InstructionCount: UINT,
    TempRegisterCount: UINT,
    TempArrayCount: UINT,
    DefCount: UINT,
    DclCount: UINT,
    TextureNormalInstructions: UINT,
    TextureLoadInstructions: UINT,
    TextureCompInstructions: UINT,
    TextureBiasInstructions: UINT,
    TextureGradientInstructions: UINT,
    FloatInstructionCount: UINT,
    IntInstructionCount: UINT,
    UintInstructionCount: UINT,
    StaticFlowControlCount: UINT,
    DynamicFlowControlCount: UINT,
    MacroInstructionCount: UINT,
    ArrayInstructionCount: UINT,
    MovInstructionCount: UINT,
    MovcInstructionCount: UINT,
    ConversionInstructionCount: UINT,
    BitwiseInstructionCount: UINT,
    MinFeatureLevel: D3D_FEATURE_LEVEL,
    RequiredFeatureFlags: UINT64,
    Name: LPCSTR,
    FunctionParameterCount: INT,
    HasReturn: BOOL,
    Has10Level9VertexShader: BOOL,
    Has10Level9PixelShader: BOOL,
}}
STRUCT!{struct D3D11_PARAMETER_DESC {
    Name: LPCSTR,
    SemanticName: LPCSTR,
    Type: D3D_SHADER_VARIABLE_TYPE,
    Class: D3D_SHADER_VARIABLE_CLASS,
    Rows: UINT,
    Columns: UINT,
    InterpolationMode: D3D_INTERPOLATION_MODE,
    Flags: D3D_PARAMETER_FLAGS,
    FirstInRegister: UINT,
    FirstInComponent: UINT,
    FirstOutRegister: UINT,
    FirstOutComponent: UINT,
}}
RIDL!{interface ID3D11ShaderReflectionType(ID3D11ShaderReflectionTypeVtbl) {
    fn GetDesc(&mut self, pDesc: *mut D3D11_SHADER_TYPE_DESC) -> HRESULT,
    fn GetMemberTypeByIndex(&mut self, Index: UINT) -> *mut ID3D11ShaderReflectionType,
    fn GetMemberTypeByName(&mut self, Name: LPCSTR) -> *mut ID3D11ShaderReflectionType,
    fn GetMemberTypeName(&mut self, Index: UINT) -> LPCSTR,
    fn IsEqual(&mut self, pType: *mut ID3D11ShaderReflectionType) -> HRESULT,
    fn GetSubType(&mut self) -> *mut ID3D11ShaderReflectionType,
    fn GetBaseClass(&mut self) -> *mut ID3D11ShaderReflectionType,
    fn GetNumInterfaces(&mut self) -> UINT,
    fn GetInterfaceByIndex(&mut self, uIndex: UINT) -> *mut ID3D11ShaderReflectionType,
    fn IsOfType(&mut self, pType: *mut ID3D11ShaderReflectionType) -> HRESULT,
    fn ImplementsInterface(&mut self, pBase: *mut ID3D11ShaderReflectionType) -> HRESULT
}}
RIDL!{interface ID3D11ShaderReflectionVariable(ID3D11ShaderReflectionVariableVtbl) {
    fn GetDesc(&mut self, pDesc: *mut D3D11_SHADER_VARIABLE_DESC) -> HRESULT,
    fn GetType(&mut self) -> *mut ID3D11ShaderReflectionType,
    fn GetBuffer(&mut self) -> *mut ID3D11ShaderReflectionConstantBuffer,
    fn GetInterfaceSlot(&mut self, uArrayIndex: UINT) -> UINT
}}
RIDL!{interface ID3D11ShaderReflectionConstantBuffer(ID3D11ShaderReflectionConstantBufferVtbl) {
    fn GetDesc(&mut self, pDesc: *mut D3D11_SHADER_BUFFER_DESC) -> HRESULT,
    fn GetVariableByIndex(&mut self, Index: UINT) -> *mut ID3D11ShaderReflectionVariable,
    fn GetVariableByName(&mut self, Name: LPCSTR) -> *mut ID3D11ShaderReflectionVariable
}}
RIDL!{interface ID3D11ShaderReflection(ID3D11ShaderReflectionVtbl): IUnknown(IUnknownVtbl) {
    fn GetDesc(&mut self, pDesc: *mut D3D11_SHADER_DESC) -> HRESULT,
    fn GetConstantBufferByIndex(
        &mut self, Index: UINT
    ) -> *mut ID3D11ShaderReflectionConstantBuffer,
    fn GetConstantBufferByName(
        &mut self, Name: LPCSTR
    ) -> *mut ID3D11ShaderReflectionConstantBuffer,
    fn GetResourceBindingDesc(
        &mut self, ResourceIndex: UINT, pDesc: *mut D3D11_SHADER_INPUT_BIND_DESC
    ) -> HRESULT,
    fn GetInputParameterDesc(
        &mut self, ParameterIndex: UINT, pDesc: *mut D3D11_SIGNATURE_PARAMETER_DESC
    ) -> HRESULT,
    fn GetOutputParameterDesc(
        &mut self, ParameterIndex: UINT, pDesc: *mut D3D11_SIGNATURE_PARAMETER_DESC
    ) -> HRESULT,
    fn GetPatchConstantParameterDesc(
        &mut self, ParameterIndex: UINT, pDesc: *mut D3D11_SIGNATURE_PARAMETER_DESC
    ) -> HRESULT,
    fn GetVariableByName(&mut self, Name: LPCSTR) -> *mut ID3D11ShaderReflectionVariable,
    fn GetResourceBindingDescByName(
        &mut self, Name: LPCSTR, pDesc: *mut D3D11_SHADER_INPUT_BIND_DESC
    ) -> HRESULT,
    fn GetMovInstructionCount(&mut self) -> UINT,
    fn GetMovcInstructionCount(&mut self) -> UINT,
    fn GetConversionInstructionCount(&mut self) -> UINT,
    fn GetBitwiseInstructionCount(&mut self) -> UINT,
    fn GetGSInputPrimitive(&mut self) -> D3D_PRIMITIVE,
    fn IsSampleFrequencyShader(&mut self) -> BOOL,
    fn GetNumInterfaceSlots(&mut self) -> UINT,
    fn GetMinFeatureLevel(&mut self, pLevel: *mut D3D_FEATURE_LEVEL) -> HRESULT,
    fn GetThreadGroupSize(
        &mut self, pSizeX: *mut UINT, pSizeY: *mut UINT, pSizeZ: *mut UINT
    ) -> UINT,
    fn GetRequiresFlags(&mut self) -> UINT64
}}
RIDL!{interface ID3D11LibraryReflection(ID3D11LibraryReflectionVtbl): IUnknown(IUnknownVtbl) {
    fn GetDesc(&mut self, pDesc: *mut D3D11_LIBRARY_DESC) -> HRESULT,
    fn GetFunctionByIndex(&mut self, FunctionIndex: INT) -> *mut ID3D11FunctionReflection
}}
RIDL!{interface ID3D11FunctionReflection(ID3D11FunctionReflectionVtbl) {
    fn GetDesc(&mut self, pDesc: *mut D3D11_FUNCTION_DESC) -> HRESULT,
    fn GetConstantBufferByIndex(
        &mut self, BufferIndex: UINT
    ) -> *mut ID3D11ShaderReflectionConstantBuffer,
    fn GetConstantBufferByName(
        &mut self, Name: LPCSTR
    ) -> *mut ID3D11ShaderReflectionConstantBuffer,
    fn GetResourceBindingDesc(
        &mut self, ResourceIndex: UINT, pDesc: *mut D3D11_SHADER_INPUT_BIND_DESC
    ) -> HRESULT,
    fn GetVariableByName(&mut self, Name: LPCSTR) -> *mut ID3D11ShaderReflectionVariable,
    fn GetResourceBindingDescByName(
        &mut self, Name: LPCSTR, pDesc: *mut D3D11_SHADER_INPUT_BIND_DESC
    ) -> HRESULT,
    fn GetFunctionParameter(
        &mut self, ParameterIndex: INT
    ) -> *mut ID3D11FunctionParameterReflection
}}
RIDL!{interface ID3D11FunctionParameterReflection(ID3D11FunctionParameterReflectionVtbl) {
    fn GetDesc(&mut self, pDesc: *mut D3D11_PARAMETER_DESC) -> HRESULT
}}
RIDL!{interface ID3D11Module(ID3D11ModuleVtbl): IUnknown(IUnknownVtbl) {
    fn CreateInstance(
        &mut self, pNamespace: LPCSTR, ppModuleInstance: *mut *mut ID3D11ModuleInstance
    ) -> HRESULT
}}
RIDL!{interface ID3D11ModuleInstance(ID3D11ModuleInstanceVtbl): IUnknown(IUnknownVtbl) {
    fn BindConstantBuffer(&mut self, uSrcSlot: UINT, uDstSlot: UINT, cbDstOffset: UINT) -> HRESULT,
    fn BindConstantBufferByName(
        &mut self, pName: LPCSTR, uDstSlot: UINT, cbDstOffset: UINT
    ) -> HRESULT,
    fn BindResource(&mut self, uSrcSlot: UINT, uDstSlot: UINT, uCount: UINT) -> HRESULT,
    fn BindResourceByName(&mut self, pName: LPCSTR, uDstSlot: UINT, uCount: UINT) -> HRESULT,
    fn BindSampler(&mut self, uSrcSlot: UINT, uDstSlot: UINT, uCount: UINT) -> HRESULT,
    fn BindSamplerByName(&mut self, pName: LPCSTR, uDstSlot: UINT, uCount: UINT) -> HRESULT,
    fn BindUnorderedAccessView(&mut self, uSrcSlot: UINT, uDstSlot: UINT, uCount: UINT) -> HRESULT,
    fn BindUnorderedAccessViewByName(
        &mut self, pName: LPCSTR, uDstSlot: UINT, uCount: UINT
    ) -> HRESULT,
    fn BindResourceAsUnorderedAccessView(
        &mut self, uSrcSrvSlot: UINT, uDstUavSlot: UINT, uCount: UINT
    ) -> HRESULT,
    fn BindResourceAsUnorderedAccessViewByName(
        &mut self, pSrvName: LPCSTR, uDstUavSlot: UINT, uCount: UINT
    ) -> HRESULT
}}
RIDL!{interface ID3D11Linker(ID3D11LinkerVtbl): IUnknown(IUnknownVtbl) {
    fn Link(
        &mut self, pEntry: *mut ID3D11ModuleInstance, pEntryName: LPCSTR, pTargetName: LPCSTR,
        uFlags: UINT, ppShaderBlob: *mut *mut ID3DBlob, ppErrorBuffer: *mut *mut ID3DBlob
    ) -> HRESULT,
    fn UseLibrary(&mut self, pLibraryMI: *mut ID3D11ModuleInstance) -> HRESULT,
    fn AddClipPlaneFromCBuffer(&mut self, uCBufferSlot: UINT, uCBufferEntry: UINT) -> HRESULT
}}
RIDL!{interface ID3D11LinkingNode(ID3D11LinkingNodeVtbl): IUnknown(IUnknownVtbl) {}}
RIDL!{interface ID3D11FunctionLinkingGraph(ID3D11FunctionLinkingGraphVtbl): IUnknown(IUnknownVtbl) {
    fn CreateModuleInstance(
        &mut self, ppModuleInstance: *mut *mut ID3D11ModuleInstance,
        ppErrorBuffer: *mut *mut ID3DBlob
    ) -> HRESULT,
    fn SetInputSignature(
        &mut self, pInputParameters: *const D3D11_PARAMETER_DESC, cInputParameters: UINT,
        ppInputNode: *mut *mut ID3D11LinkingNode
    ) -> HRESULT,
    fn SetOutputSignature(
        &mut self, pOutputParameters: *const D3D11_PARAMETER_DESC, cOutputParameters: UINT,
        ppOutputNode: *mut *mut ID3D11LinkingNode
    ) -> HRESULT,
    fn CallFunction(
        &mut self, pModuleInstanceNamespace: LPCSTR,
        pModuleWithFunctionPrototype: *mut ID3D11Module, pFunctionName: LPCSTR,
        ppCallNode: *mut *mut ID3D11LinkingNode
    ) -> HRESULT,
    fn PassValue(
        &mut self, pSrcNode: *mut ID3D11LinkingNode, SrcParameterIndex: INT,
        pDstNode: *mut ID3D11LinkingNode, DstParameterIndex: INT
    ) -> HRESULT,
    fn PassValueWithSwizzle(
        &mut self, pSrcNode: *mut ID3D11LinkingNode, SrcParameterIndex: INT, pSrcSwizzle: LPCSTR,
        pDstNode: *mut ID3D11LinkingNode, DstParameterIndex: INT, pDstSwizzle: LPCSTR
    ) -> HRESULT,
    fn GetLastError(&mut self, ppErrorBuffer: *mut *mut ID3DBlob) -> HRESULT,
    fn GenerateHlsl(&mut self, uFlags: UINT, ppBuffer: *mut *mut ID3DBlob) -> HRESULT
}}

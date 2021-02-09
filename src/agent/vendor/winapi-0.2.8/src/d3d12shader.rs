// Copyright © 2016; Dmitry Roschin
// Licensed under the MIT License <LICENSE.md>
FLAGS!{ enum D3D12_SHADER_VERSION_TYPE {
    D3D12_SHVER_PIXEL_SHADER = 0x0,
    D3D12_SHVER_VERTEX_SHADER = 0x1,
    D3D12_SHVER_GEOMETRY_SHADER = 0x2,
    D3D12_SHVER_HULL_SHADER = 0x3,
    D3D12_SHVER_DOMAIN_SHADER = 0x4,
    D3D12_SHVER_COMPUTE_SHADER = 0x5,
    D3D12_SHVER_RESERVED0 = 0xFFF0,
}}

STRUCT!{struct D3D12_FUNCTION_DESC {
    Version: ::UINT,
    Creator: ::LPCSTR,
    Flags: ::UINT,
    ConstantBuffers: ::UINT,
    BoundResources: ::UINT,
    InstructionCount: ::UINT,
    TempRegisterCount: ::UINT,
    TempArrayCount: ::UINT,
    DefCount: ::UINT,
    DclCount: ::UINT,
    TextureNormalInstructions: ::UINT,
    TextureLoadInstructions: ::UINT,
    TextureCompInstructions: ::UINT,
    TextureBiasInstructions: ::UINT,
    TextureGradientInstructions: ::UINT,
    FloatInstructionCount: ::UINT,
    IntInstructionCount: ::UINT,
    UintInstructionCount: ::UINT,
    StaticFlowControlCount: ::UINT,
    DynamicFlowControlCount: ::UINT,
    MacroInstructionCount: ::UINT,
    ArrayInstructionCount: ::UINT,
    MovInstructionCount: ::UINT,
    MovcInstructionCount: ::UINT,
    ConversionInstructionCount: ::UINT,
    BitwiseInstructionCount: ::UINT,
    MinFeatureLevel: ::D3D_FEATURE_LEVEL,
    RequiredFeatureFlags: ::UINT64,
    Name: ::LPCSTR,
    FunctionParameterCount: ::INT,
    HasReturn: ::BOOL,
    Has10Level9VertexShader: ::BOOL,
    Has10Level9PixelShader: ::BOOL,
}}

STRUCT!{struct D3D12_LIBRARY_DESC {
    Creator: ::LPCSTR,
    Flags: ::UINT,
    FunctionCount: ::UINT,
}}

STRUCT!{struct D3D12_PARAMETER_DESC {
    Name: ::LPCSTR,
    SemanticName: ::LPCSTR,
    Type: ::D3D_SHADER_VARIABLE_TYPE,
    Class: ::D3D_SHADER_VARIABLE_CLASS,
    Rows: ::UINT,
    Columns: ::UINT,
    InterpolationMode: ::D3D_INTERPOLATION_MODE,
    Flags: ::D3D_PARAMETER_FLAGS,
    FirstInRegister: ::UINT,
    FirstInComponent: ::UINT,
    FirstOutRegister: ::UINT,
    FirstOutComponent: ::UINT,
}}

STRUCT!{struct D3D12_SHADER_BUFFER_DESC {
    Name: ::LPCSTR,
    Type: ::D3D_CBUFFER_TYPE,
    Variables: ::UINT,
    Size: ::UINT,
    uFlags: ::UINT,
}}

STRUCT!{struct D3D12_SHADER_DESC {
    Version: ::UINT,
    Creator: ::LPCSTR,
    Flags: ::UINT,
    ConstantBuffers: ::UINT,
    BoundResources: ::UINT,
    InputParameters: ::UINT,
    OutputParameters: ::UINT,
    InstructionCount: ::UINT,
    TempRegisterCount: ::UINT,
    TempArrayCount: ::UINT,
    DefCount: ::UINT,
    DclCount: ::UINT,
    TextureNormalInstructions: ::UINT,
    TextureLoadInstructions: ::UINT,
    TextureCompInstructions: ::UINT,
    TextureBiasInstructions: ::UINT,
    TextureGradientInstructions: ::UINT,
    FloatInstructionCount: ::UINT,
    IntInstructionCount: ::UINT,
    UintInstructionCount: ::UINT,
    StaticFlowControlCount: ::UINT,
    DynamicFlowControlCount: ::UINT,
    MacroInstructionCount: ::UINT,
    ArrayInstructionCount: ::UINT,
    CutInstructionCount: ::UINT,
    EmitInstructionCount: ::UINT,
    GSOutputTopology: ::D3D_PRIMITIVE_TOPOLOGY,
    GSMaxOutputVertexCount: ::UINT,
    InputPrimitive: ::D3D_PRIMITIVE,
    PatchConstantParameters: ::UINT,
    cGSInstanceCount: ::UINT,
    cControlPoints: ::UINT,
    HSOutputPrimitive: ::D3D_TESSELLATOR_OUTPUT_PRIMITIVE,
    HSPartitioning: ::D3D_TESSELLATOR_PARTITIONING,
    TessellatorDomain: ::D3D_TESSELLATOR_DOMAIN,
    cBarrierInstructions: ::UINT,
    cInterlockedInstructions: ::UINT,
    cTextureStoreInstructions: ::UINT,
}}

STRUCT!{struct D3D12_SHADER_INPUT_BIND_DESC {
    Name: ::LPCSTR,
    Type: ::D3D_SHADER_INPUT_TYPE,
    BindPoint: ::UINT,
    BindCount: ::UINT,
    uFlags: ::UINT,
    ReturnType: ::D3D_RESOURCE_RETURN_TYPE,
    Dimension: ::D3D_SRV_DIMENSION,
    NumSamples: ::UINT,
    Space: ::UINT,
    uID: ::UINT,
}}

STRUCT!{struct D3D12_SHADER_TYPE_DESC {
    Class: ::D3D_SHADER_VARIABLE_CLASS,
    Type: ::D3D_SHADER_VARIABLE_TYPE,
    Rows: ::UINT,
    Columns: ::UINT,
    Elements: ::UINT,
    Members: ::UINT,
    Offset: ::UINT,
    Name: ::LPCSTR,
}}

STRUCT!{struct D3D12_SHADER_VARIABLE_DESC {
    Name: ::LPCSTR,
    StartOffset: ::UINT,
    Size: ::UINT,
    uFlags: ::UINT,
    DefaultValue: ::LPVOID,
    StartTexture: ::UINT,
    TextureSize: ::UINT,
    StartSampler: ::UINT,
    SamplerSize: ::UINT,
}}

STRUCT!{struct D3D12_SIGNATURE_PARAMETER_DESC {
    SemanticName: ::LPCSTR,
    SemanticIndex: ::UINT,
    Register: ::UINT,
    SystemValueType: ::D3D_NAME,
    ComponentType: ::D3D_REGISTER_COMPONENT_TYPE,
    Mask: ::BYTE,
    ReadWriteMask: ::BYTE,
    Stream: ::UINT,
    MinPrecision: ::D3D_MIN_PRECISION,
}}

RIDL!(
interface ID3D12FunctionParameterReflection(ID3D12FunctionParameterReflectionVtbl) {
    fn GetDesc(&mut self, pDesc: *mut ::D3D12_PARAMETER_DESC) -> ::HRESULT
});

RIDL!(
interface ID3D12FunctionReflection(ID3D12FunctionReflectionVtbl) {
    fn GetDesc(&mut self, pDesc: *mut ::D3D12_FUNCTION_DESC) -> ::HRESULT,
    fn GetConstantBufferByIndex(
        &mut self, BufferIndex: ::UINT
    ) -> *mut ::ID3D12ShaderReflectionConstantBuffer,
    fn GetConstantBufferByName(
        &mut self, Name: ::LPCSTR
    ) -> *mut ::ID3D12ShaderReflectionConstantBuffer,
    fn GetResourceBindingDesc(
        &mut self, ResourceIndex: ::UINT, pDesc: *mut ::D3D12_SHADER_INPUT_BIND_DESC
    ) -> ::HRESULT,
    fn GetVariableByName(
        &mut self, Name: ::LPCSTR
    ) -> *mut ::ID3D12ShaderReflectionVariable,
    fn GetResourceBindingDescByName(
        &mut self, Name: ::LPCSTR, pDesc: *mut ::D3D12_SHADER_INPUT_BIND_DESC
    ) -> ::HRESULT,
    fn GetFunctionParameter(
        &mut self, ParameterIndex: ::INT
    ) -> *mut ::ID3D12FunctionParameterReflection
});

RIDL!(
interface ID3D12LibraryReflection(ID3D12LibraryReflectionVtbl): IUnknown(IUnknownVtbl) {
    fn QueryInterface(
        &mut self, iid: *const ::IID, ppv: *mut ::LPVOID
    ) -> ::HRESULT,
    fn AddRef(&mut self) -> ::ULONG,
    fn Release(&mut self) -> ::ULONG,
    fn GetDesc(&mut self, pDesc: *mut ::D3D12_LIBRARY_DESC) -> ::HRESULT,
    fn GetFunctionByIndex(
        &mut self, FunctionIndex: ::INT
    ) -> *mut ::ID3D12FunctionReflection
});

RIDL!(
interface ID3D12ShaderReflectionConstantBuffer(ID3D12ShaderReflectionConstantBufferVtbl) {
    fn GetDesc(&mut self, pDesc: *mut ::D3D12_SHADER_BUFFER_DESC) -> ::HRESULT,
    fn GetVariableByIndex(
        &mut self, Index: ::UINT
    ) -> *mut ::ID3D12ShaderReflectionVariable,
    fn GetVariableByName(
        &mut self, Name: ::LPCSTR
    ) -> *mut ::ID3D12ShaderReflectionVariable
});

RIDL!(
interface ID3D12ShaderReflectionType(ID3D12ShaderReflectionTypeVtbl) {
    fn GetDesc(&mut self, pDesc: *mut ::D3D12_SHADER_TYPE_DESC) -> ::HRESULT,
    fn GetMemberTypeByIndex(
        &mut self, Index: ::UINT
    ) -> *mut ::ID3D12ShaderReflectionType,
    fn GetMemberTypeByName(
        &mut self, Name: ::LPCSTR
    ) -> *mut ::ID3D12ShaderReflectionType,
    fn GetMemberTypeName(&mut self, Index: ::UINT) -> ::LPCSTR,
    fn IsEqual(
        &mut self, pType: *mut ::ID3D12ShaderReflectionType
    ) -> ::HRESULT,
    fn GetSubType(&mut self) -> *mut ::ID3D12ShaderReflectionType,
    fn GetBaseClass(&mut self) -> *mut ::ID3D12ShaderReflectionType,
    fn GetNumInterfaces(&mut self) -> ::UINT,
    fn GetInterfaceByIndex(
        &mut self, uIndex: ::UINT
    ) -> *mut ::ID3D12ShaderReflectionType,
    fn IsOfType(
        &mut self, pType: *mut ::ID3D12ShaderReflectionType
    ) -> ::HRESULT,
    fn ImplementsInterface(
        &mut self, pBase: *mut ::ID3D12ShaderReflectionType
    ) -> ::HRESULT
});

RIDL!(
interface ID3D12ShaderReflectionVariable(ID3D12ShaderReflectionVariableVtbl) {
    fn GetDesc(
        &mut self, pDesc: *mut ::D3D12_SHADER_VARIABLE_DESC
    ) -> ::HRESULT,
    fn GetType(&mut self) -> *mut ::ID3D12ShaderReflectionType,
    fn GetBuffer(&mut self) -> *mut ::ID3D12ShaderReflectionConstantBuffer,
    fn GetInterfaceSlot(&mut self, uArrayIndex: ::UINT) -> ::UINT
});

RIDL!(
interface ID3D12ShaderReflection(ID3D12ShaderReflectionVtbl): IUnknown(IUnknownVtbl) {
    fn QueryInterface(
        &mut self, iid: *const ::IID, ppv: *mut ::LPVOID
    ) -> ::HRESULT,
    fn AddRef(&mut self) -> ::ULONG,
    fn Release(&mut self) -> ::ULONG,
    fn GetDesc(&mut self, pDesc: *mut ::D3D12_SHADER_DESC) -> ::HRESULT,
    fn GetConstantBufferByIndex(
        &mut self, Index: ::UINT
    ) -> *mut ::ID3D12ShaderReflectionConstantBuffer,
    fn GetConstantBufferByName(
        &mut self, Name: ::LPCSTR
    ) -> *mut ::ID3D12ShaderReflectionConstantBuffer,
    fn GetResourceBindingDesc(
        &mut self, ResourceIndex: ::UINT, pDesc: *mut ::D3D12_SHADER_INPUT_BIND_DESC
    ) -> ::HRESULT,
    fn GetInputParameterDesc(
        &mut self, ParameterIndex: ::UINT, pDesc: *mut ::D3D12_SIGNATURE_PARAMETER_DESC
    ) -> ::HRESULT,
    fn GetOutputParameterDesc(
        &mut self, ParameterIndex: ::UINT, pDesc: *mut ::D3D12_SIGNATURE_PARAMETER_DESC
    ) -> ::HRESULT,
    fn GetPatchConstantParameterDesc(
        &mut self, ParameterIndex: ::UINT, pDesc: *mut ::D3D12_SIGNATURE_PARAMETER_DESC
    ) -> ::HRESULT,
    fn GetVariableByName(
        &mut self, Name: ::LPCSTR
    ) -> *mut ::ID3D12ShaderReflectionVariable,
    fn GetResourceBindingDescByName(
        &mut self, Name: ::LPCSTR, pDesc: *mut ::D3D12_SHADER_INPUT_BIND_DESC
    ) -> ::HRESULT,
    fn GetMovInstructionCount(&mut self) -> ::UINT,
    fn GetMovcInstructionCount(&mut self) -> ::UINT,
    fn GetConversionInstructionCount(&mut self) -> ::UINT,
    fn GetBitwiseInstructionCount(&mut self) -> ::UINT,
    fn GetGSInputPrimitive(&mut self) -> ::D3D_PRIMITIVE,
    fn IsSampleFrequencyShader(&mut self) -> ::BOOL,
    fn GetNumInterfaceSlots(&mut self) -> ::UINT,
    fn GetMinFeatureLevel(
        &mut self, pLevel: *mut ::D3D_FEATURE_LEVEL
    ) -> ::HRESULT,
    fn GetThreadGroupSize(
        &mut self, pSizeX: *mut ::UINT, pSizeY: *mut ::UINT, pSizeZ: *mut ::UINT
    ) -> ::UINT,
    fn GetRequiresFlags(&mut self) -> ::UINT64
});

pub type D3D12_CBUFFER_TYPE = ::D3D_CBUFFER_TYPE;
pub type D3D12_RESOURCE_RETURN_TYPE = ::D3D_RESOURCE_RETURN_TYPE;
pub type D3D12_TESSELLATOR_DOMAIN = ::D3D_TESSELLATOR_DOMAIN;
pub type D3D12_TESSELLATOR_OUTPUT_PRIMITIVE = ::D3D_TESSELLATOR_OUTPUT_PRIMITIVE;
pub type D3D12_TESSELLATOR_PARTITIONING = ::D3D_TESSELLATOR_PARTITIONING;
pub type LPD3D12FUNCTIONPARAMETERREFLECTION = *mut ::ID3D12FunctionParameterReflection;
pub type LPD3D12FUNCTIONREFLECTION = *mut ::ID3D12FunctionReflection;
pub type LPD3D12LIBRARYREFLECTION = *mut ::ID3D12LibraryReflection;
pub type LPD3D12SHADERREFLECTION = *mut ::ID3D12ShaderReflection;
pub type LPD3D12SHADERREFLECTIONCONSTANTBUFFER = *mut ::ID3D12ShaderReflectionConstantBuffer;
pub type LPD3D12SHADERREFLECTIONTYPE = *mut ::ID3D12ShaderReflectionType;
pub type LPD3D12SHADERREFLECTIONVARIABLE = *mut ::ID3D12ShaderReflectionVariable;
pub const D3D_SHADER_REQUIRES_INNER_COVERAGE: ::UINT64 = 0x00000400;
pub const D3D_SHADER_REQUIRES_ROVS: ::UINT64 = 0x00001000;
pub const D3D_SHADER_REQUIRES_STENCIL_REF: ::UINT64 = 0x00000200;
pub const D3D_SHADER_REQUIRES_TYPED_UAV_LOAD_ADDITIONAL_FORMATS: ::UINT64 = 0x00000800;
pub const D3D_SHADER_REQUIRES_VIEWPORT_AND_RT_ARRAY_INDEX_FROM_ANY_SHADER_FEEDING_RASTERIZER: ::UINT64 = 0x00002000;

// Copyright © 2016, Peter Atashian
// Licensed under the MIT License <LICENSE.md>
use super::*;
pub type D3D10_RESOURCE_RETURN_TYPE = D3D_RESOURCE_RETURN_TYPE;
pub type D3D10_CBUFFER_TYPE = D3D_CBUFFER_TYPE;
STRUCT!{struct D3D10_SHADER_DESC {
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
}}
STRUCT!{struct D3D10_SHADER_BUFFER_DESC {
    Name: LPCSTR,
    Type: D3D10_CBUFFER_TYPE,
    Variables: UINT,
    Size: UINT,
    uFlags: UINT,
}}
STRUCT!{struct D3D10_SHADER_VARIABLE_DESC {
    Name: LPCSTR,
    StartOffset: UINT,
    Size: UINT,
    uFlags: UINT,
    DefaultValue: LPVOID,
}}
STRUCT!{struct D3D10_SHADER_TYPE_DESC {
    Class: D3D_SHADER_VARIABLE_CLASS,
    Type: D3D_SHADER_VARIABLE_TYPE,
    Rows: UINT,
    Columns: UINT,
    Elements: UINT,
    Members: UINT,
    Offset: UINT,
}}
STRUCT!{struct D3D10_SHADER_INPUT_BIND_DESC {
    Name: LPCSTR,
    Type: D3D_SHADER_INPUT_TYPE,
    BindPoint: UINT,
    BindCount: UINT,
    uFlags: UINT,
    ReturnType: D3D_RESOURCE_RETURN_TYPE,
    Dimension: D3D_SRV_DIMENSION,
    NumSamples: UINT,
}}
STRUCT!{struct D3D10_SIGNATURE_PARAMETER_DESC {
    SemanticName: LPCSTR,
    SemanticIndex: UINT,
    Register: UINT,
    SystemValueType: D3D_NAME,
    ComponentType: D3D_REGISTER_COMPONENT_TYPE,
    Mask: BYTE,
    ReadWriteMask: BYTE,
}}
RIDL!{interface ID3D10ShaderReflectionType(ID3D10ShaderReflectionTypeVtbl) {
    fn GetDesc(&mut self, pDesc: *mut D3D10_SHADER_TYPE_DESC) -> HRESULT,
    fn GetMemberTypeByIndex(&mut self, Index: UINT) -> *mut ID3D10ShaderReflectionType,
    fn GetMemberTypeByName(&mut self, Name: LPCSTR) -> *mut ID3D10ShaderReflectionType,
    fn GetMemberTypeName(&mut self, Index: UINT) -> LPCSTR
}}
RIDL!{interface ID3D10ShaderReflectionVariable(ID3D10ShaderReflectionVariableVtbl) {
    fn GetDesc(&mut self, pDesc: *mut D3D10_SHADER_VARIABLE_DESC) -> HRESULT,
    fn GetType(&mut self) -> *mut ID3D10ShaderReflectionType
}}
RIDL!{interface ID3D10ShaderReflectionConstantBuffer(ID3D10ShaderReflectionConstantBufferVtbl) {
    fn GetDesc(&mut self, pDesc: *mut D3D10_SHADER_BUFFER_DESC) -> HRESULT,
    fn GetVariableByIndex(&mut self, Index: UINT) -> *mut ID3D10ShaderReflectionVariable,
    fn GetVariableByName(&mut self, Name: LPCSTR) -> *mut ID3D10ShaderReflectionVariable
}}
RIDL!{interface ID3D10ShaderReflection(ID3D10ShaderReflectionVtbl): IUnknown(IUnknownVtbl) {
    fn GetDesc(&mut self, pDesc: *mut D3D10_SHADER_DESC) -> HRESULT,
    fn GetConstantBufferByIndex(
        &mut self, Index: UINT
    ) -> *mut ID3D10ShaderReflectionConstantBuffer,
    fn GetConstantBufferByName(
        &mut self, Name: LPCSTR
    ) -> *mut ID3D10ShaderReflectionConstantBuffer,
    fn GetResourceBindingDesc(
        &mut self, ResourceIndex: UINT, pDesc: *mut D3D10_SHADER_INPUT_BIND_DESC
    ) -> HRESULT,
    fn GetInputParameterDesc(
        &mut self, ParameterIndex: UINT, pDesc: *mut D3D10_SIGNATURE_PARAMETER_DESC
    ) -> HRESULT,
    fn GetOutputParameterDesc(
        &mut self, ParameterIndex: UINT, pDesc: *mut D3D10_SIGNATURE_PARAMETER_DESC
    ) -> HRESULT
}}

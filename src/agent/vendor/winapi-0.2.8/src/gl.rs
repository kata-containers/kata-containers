// Copyright © 2015, Peter Atashian
// Licensed under the MIT License <LICENSE.md>
//48
pub type GLenum = ::c_uint;
pub type GLboolean = ::c_uchar;
pub type GLbitfield = ::c_uint;
pub type GLbyte = ::c_schar;
pub type GLshort = ::c_short;
pub type GLint = ::c_int;
pub type GLsizei = ::c_int;
pub type GLubyte = ::c_uchar;
pub type GLushort = ::c_ushort;
pub type GLuint = ::c_uint;
pub type GLfloat = ::c_float;
pub type GLclampf = ::c_float;
pub type GLdouble = ::c_double;
pub type GLclampd = ::c_double;
pub type GLvoid = ::c_void;
//63
//68
//AccumOp
pub const GL_ACCUM: GLenum = 0x0100;
pub const GL_LOAD: GLenum = 0x0101;
pub const GL_RETURN: GLenum = 0x0102;
pub const GL_MULT: GLenum = 0x0103;
pub const GL_ADD: GLenum = 0x0104;
//AlphaFunction
pub const GL_NEVER: GLenum = 0x0200;
pub const GL_LESS: GLenum = 0x0201;
pub const GL_EQUAL: GLenum = 0x0202;
pub const GL_LEQUAL: GLenum = 0x0203;
pub const GL_GREATER: GLenum = 0x0204;
pub const GL_NOTEQUAL: GLenum = 0x0205;
pub const GL_GEQUAL: GLenum = 0x0206;
pub const GL_ALWAYS: GLenum = 0x0207;

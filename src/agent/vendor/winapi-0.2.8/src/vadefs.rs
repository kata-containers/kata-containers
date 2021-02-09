// Copyright © 2015, Peter Atashian
// Licensed under the MIT License <LICENSE.md>
//! Definitions of macro helpers used by <stdarg.h>.  This is the topmost header in the CRT header
//! lattice, and is always the first CRT header to be included, explicitly or implicitly.
//! Therefore, this header also has several definitions that are used throughout the CRT.
//39
pub type va_list = *mut ::c_char;

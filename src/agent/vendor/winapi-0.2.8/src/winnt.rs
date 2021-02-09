// Copyright © 2015, Peter Atashian
// Licensed under the MIT License <LICENSE.md>
//! This module defines the 32-Bit Windows types and constants that are defined by NT, but exposed
//! through the Win32 API.
pub const ANYSIZE_ARRAY: usize = 1;
//341
pub type PVOID = *mut ::c_void;
pub type PVOID64 = u64; // This is a 64-bit pointer, even when in 32-bit
//382
pub type VOID = ::c_void;
pub type CHAR = ::c_char;
pub type SHORT = ::c_short;
pub type LONG = ::c_long;
// pub type INT = ::c_int; // Already defined by minwindef.h
pub type WCHAR = ::wchar_t;
pub type PWCHAR = *mut WCHAR;
pub type LPWCH = *mut WCHAR;
pub type PWCH = *mut WCHAR;
pub type LPCWCH = *const WCHAR;
pub type PCWCH = *const WCHAR;
pub type NWPSTR = *mut WCHAR;
pub type LPWSTR = *mut WCHAR;
pub type PWSTR = *mut WCHAR;
pub type PZPWSTR = *mut PWSTR;
pub type PCZPWSTR = *const PWSTR;
pub type LPUWSTR = *mut WCHAR;
pub type PUWSTR = *mut WCHAR;
pub type LPCWSTR = *const WCHAR;
pub type PCWSTR = *const WCHAR;
pub type PZPCWSTR= *mut PCWSTR;
pub type PCZPCWSTR = *const PCWSTR;
pub type LPCUWSTR = *const WCHAR;
pub type PCUWSTR = *const WCHAR;
pub type PZZWSTR= *mut WCHAR;
pub type PCZZWSTR = *const WCHAR;
pub type PUZZWSTR = *mut WCHAR;
pub type PCUZZWSTR = *const WCHAR;
pub type PNZWCH = *mut WCHAR;
pub type PCNZWCH = *const WCHAR;
pub type PUNZWCH = *mut WCHAR;
pub type PCUNZWCH = *const WCHAR;
pub type LPCWCHAR = *const WCHAR;
pub type PCWCHAR = *const WCHAR;
pub type LPCUWCHAR = *const WCHAR;
pub type PCUWCHAR = *const WCHAR;
pub type UCSCHAR = ::c_ulong;
pub type PUCSCHAR = *mut UCSCHAR;
pub type PCUCSCHAR = *const UCSCHAR;
pub type PUCSSTR = *mut UCSCHAR;
pub type PUUCSSTR = *mut UCSCHAR;
pub type PCUCSSTR = *const UCSCHAR;
pub type PCUUCSSTR = *const UCSCHAR;
pub type PUUCSCHAR = *mut UCSCHAR;
pub type PCUUCSCHAR = *const UCSCHAR;
pub type PCHAR = *mut CHAR;
pub type LPCH = *mut CHAR;
pub type PCH = *mut CHAR;
pub type LPCCH = *const CHAR;
pub type PCCH = *const CHAR;
pub type NPSTR = *mut CHAR;
pub type LPSTR = *mut CHAR;
pub type PSTR = *mut CHAR;
pub type PZPSTR = *mut PSTR;
pub type PCZPSTR = *const PSTR;
pub type LPCSTR = *const CHAR;
pub type PCSTR = *const CHAR;
pub type PZPCSTR = *mut PCSTR;
pub type PCZPCSTR = *const PCSTR;
pub type PZZSTR = *mut CHAR;
pub type PCZZSTR = *const CHAR;
pub type PNZCH = *mut CHAR;
pub type PCNZCH = *const CHAR;
// Skipping TCHAR things
pub type PSHORT = *mut SHORT;
pub type PLONG = *mut LONG;
STRUCT!{struct PROCESSOR_NUMBER {
    Group: ::WORD,
    Number: ::BYTE,
    Reserved: ::BYTE,
}}
pub type PPROCESSOR_NUMBER = *mut PROCESSOR_NUMBER;
STRUCT!{struct GROUP_AFFINITY {
    Mask: ::KAFFINITY,
    Group: ::WORD,
    Reserved: [::WORD; 3],
}}
pub type PGROUP_AFFINITY = *mut GROUP_AFFINITY;
pub type HANDLE = *mut ::c_void;
pub type PHANDLE = *mut HANDLE;
pub type FCHAR = ::BYTE;
pub type FSHORT = ::WORD;
pub type FLONG = ::DWORD;
//667
pub type CCHAR = ::c_char;
pub type LCID = ::DWORD;
pub type PLCID = ::PDWORD;
pub type LANGID = ::WORD;
ENUM!{enum COMPARTMENT_ID {
    UNSPECIFIED_COMPARTMENT_ID = 0,
    DEFAULT_COMPARTMENT_ID = 1,
}}
pub type PCOMPARTMENT_ID = *mut COMPARTMENT_ID;
pub const APPLICATION_ERROR_MASK: ::DWORD = 0x20000000;
pub const ERROR_SEVERITY_SUCCESS: ::DWORD = 0x00000000;
pub const ERROR_SEVERITY_INFORMATIONAL: ::DWORD = 0x40000000;
pub const ERROR_SEVERITY_WARNING: ::DWORD = 0x80000000;
pub const ERROR_SEVERITY_ERROR: ::DWORD = 0xC0000000;
//710
STRUCT!{struct FLOAT128 {
    LowPart: ::__int64,
    HighPart: ::__int64,
}}
pub type PFLOAT128 = *mut FLOAT128;
pub type LONGLONG = ::__int64;
pub type ULONGLONG = ::__uint64;
pub type PLONGLONG = *mut LONGLONG;
pub type PULONGLONG = *mut ULONGLONG;
pub type USN = LONGLONG;
pub type LARGE_INTEGER = LONGLONG;
pub type PLARGE_INTEGER = *mut LARGE_INTEGER;
pub type ULARGE_INTEGER = ULONGLONG;
pub type PULARGE_INTEGER= *mut ULARGE_INTEGER;
pub type RTL_REFERENCE_COUNT = ::LONG_PTR;
pub type PRTL_REFERENCE_COUNT = *mut ::LONG_PTR;
STRUCT!{struct LUID {
    LowPart: ::DWORD,
    HighPart: LONG,
}}
pub type PLUID = *mut LUID;
pub type DWORDLONG = ULONGLONG;
pub type PDWORDLONG = *mut DWORDLONG;
//1042
pub type BOOLEAN = ::BYTE;
pub type PBOOLEAN = *mut BOOLEAN;
STRUCT!{struct LIST_ENTRY {
    Flink: *mut LIST_ENTRY,
    Blink: *mut LIST_ENTRY,
}}
pub type PLIST_ENTRY = *mut LIST_ENTRY;
STRUCT!{struct SINGLE_LIST_ENTRY {
    Next: *mut SINGLE_LIST_ENTRY,
}}
pub type PSINGLE_LIST_ENTRY = *mut SINGLE_LIST_ENTRY;
STRUCT!{struct LIST_ENTRY32 {
    Flink: ::DWORD,
    Blink: ::DWORD,
}}
pub type PLIST_ENTRY32 = *mut LIST_ENTRY32;
STRUCT!{struct LIST_ENTRY64 {
    Flink: ULONGLONG,
    Blink: ULONGLONG,
}}
pub type PLIST_ENTRY64 = *mut LIST_ENTRY64;
STRUCT!{struct OBJECTID {
    Lineage: ::GUID,
    Uniquifier: ::DWORD,
}}
pub const MINCHAR: ::CHAR = 0x80u8 as ::CHAR;
pub const MAXCHAR: ::CHAR = 0x7f;
pub const MINSHORT: ::SHORT = 0x8000u16 as ::SHORT;
pub const MAXSHORT: ::SHORT = 0x7fff;
pub const MINLONG: ::LONG = 0x80000000u32 as ::LONG;
pub const MAXLONG: ::LONG = 0x7fffffff;
pub const MAXBYTE: ::BYTE = 0xff;
pub const MAXWORD: ::WORD = 0xffff;
pub const MAXDWORD: ::DWORD = 0xffffffff;
//1300
pub type PEXCEPTION_ROUTINE = Option<unsafe extern "system" fn(
    ExceptionRecord: *mut EXCEPTION_RECORD, EstablisherFrame: PVOID, ContextRecord: *mut CONTEXT,
    DispatcherContext: PVOID,
) -> ::EXCEPTION_DISPOSITION>;
//1498
pub const LANG_NEUTRAL: ::WORD = 0x00;
pub const LANG_INVARIANT: ::WORD = 0x7f;
pub const LANG_AFRIKAANS: ::WORD = 0x36;
pub const LANG_ALBANIAN: ::WORD = 0x1c;
pub const LANG_ALSATIAN: ::WORD = 0x84;
pub const LANG_AMHARIC: ::WORD = 0x5e;
pub const LANG_ARABIC: ::WORD = 0x01;
pub const LANG_ARMENIAN: ::WORD = 0x2b;
pub const LANG_ASSAMESE: ::WORD = 0x4d;
pub const LANG_AZERI: ::WORD = 0x2c;
pub const LANG_AZERBAIJANI: ::WORD = 0x2c;
pub const LANG_BANGLA: ::WORD = 0x45;
pub const LANG_BASHKIR: ::WORD = 0x6d;
pub const LANG_BASQUE: ::WORD = 0x2d;
pub const LANG_BELARUSIAN: ::WORD = 0x23;
pub const LANG_BENGALI: ::WORD = 0x45;
pub const LANG_BRETON: ::WORD = 0x7e;
pub const LANG_BOSNIAN: ::WORD = 0x1a;
pub const LANG_BOSNIAN_NEUTRAL: ::WORD = 0x781a;
pub const LANG_BULGARIAN: ::WORD = 0x02;
pub const LANG_CATALAN: ::WORD = 0x03;
pub const LANG_CENTRAL_KURDISH: ::WORD = 0x92;
pub const LANG_CHEROKEE: ::WORD = 0x5c;
pub const LANG_CHINESE: ::WORD = 0x04;
pub const LANG_CHINESE_SIMPLIFIED: ::WORD = 0x04;
pub const LANG_CHINESE_TRADITIONAL: ::WORD = 0x7c04;
pub const LANG_CORSICAN: ::WORD = 0x83;
pub const LANG_CROATIAN: ::WORD = 0x1a;
pub const LANG_CZECH: ::WORD = 0x05;
pub const LANG_DANISH: ::WORD = 0x06;
pub const LANG_DARI: ::WORD = 0x8c;
pub const LANG_DIVEHI: ::WORD = 0x65;
pub const LANG_DUTCH: ::WORD = 0x13;
pub const LANG_ENGLISH: ::WORD = 0x09;
pub const LANG_ESTONIAN: ::WORD = 0x25;
pub const LANG_FAEROESE: ::WORD = 0x38;
pub const LANG_FARSI: ::WORD = 0x29;
pub const LANG_FILIPINO: ::WORD = 0x64;
pub const LANG_FINNISH: ::WORD = 0x0b;
pub const LANG_FRENCH: ::WORD = 0x0c;
pub const LANG_FRISIAN: ::WORD = 0x62;
pub const LANG_FULAH: ::WORD = 0x67;
pub const LANG_GALICIAN: ::WORD = 0x56;
pub const LANG_GEORGIAN: ::WORD = 0x37;
pub const LANG_GERMAN: ::WORD = 0x07;
pub const LANG_GREEK: ::WORD = 0x08;
pub const LANG_GREENLANDIC: ::WORD = 0x6f;
pub const LANG_GUJARATI: ::WORD = 0x47;
pub const LANG_HAUSA: ::WORD = 0x68;
pub const LANG_HAWAIIAN: ::WORD = 0x75;
pub const LANG_HEBREW: ::WORD = 0x0d;
pub const LANG_HINDI: ::WORD = 0x39;
pub const LANG_HUNGARIAN: ::WORD = 0x0e;
pub const LANG_ICELANDIC: ::WORD = 0x0f;
pub const LANG_IGBO: ::WORD = 0x70;
pub const LANG_INDONESIAN: ::WORD = 0x21;
pub const LANG_INUKTITUT: ::WORD = 0x5d;
pub const LANG_IRISH: ::WORD = 0x3c;
pub const LANG_ITALIAN: ::WORD = 0x10;
pub const LANG_JAPANESE: ::WORD = 0x11;
pub const LANG_KANNADA: ::WORD = 0x4b;
pub const LANG_KASHMIRI: ::WORD = 0x60;
pub const LANG_KAZAK: ::WORD = 0x3f;
pub const LANG_KHMER: ::WORD = 0x53;
pub const LANG_KICHE: ::WORD = 0x86;
pub const LANG_KINYARWANDA: ::WORD = 0x87;
pub const LANG_KONKANI: ::WORD = 0x57;
pub const LANG_KOREAN: ::WORD = 0x12;
pub const LANG_KYRGYZ: ::WORD = 0x40;
pub const LANG_LAO: ::WORD = 0x54;
pub const LANG_LATVIAN: ::WORD = 0x26;
pub const LANG_LITHUANIAN: ::WORD = 0x27;
pub const LANG_LOWER_SORBIAN: ::WORD = 0x2e;
pub const LANG_LUXEMBOURGISH: ::WORD = 0x6e;
pub const LANG_MACEDONIAN: ::WORD = 0x2f;
pub const LANG_MALAY: ::WORD = 0x3e;
pub const LANG_MALAYALAM: ::WORD = 0x4c;
pub const LANG_MALTESE: ::WORD = 0x3a;
pub const LANG_MANIPURI: ::WORD = 0x58;
pub const LANG_MAORI: ::WORD = 0x81;
pub const LANG_MAPUDUNGUN: ::WORD = 0x7a;
pub const LANG_MARATHI: ::WORD = 0x4e;
pub const LANG_MOHAWK: ::WORD = 0x7c;
pub const LANG_MONGOLIAN: ::WORD = 0x50;
pub const LANG_NEPALI: ::WORD = 0x61;
pub const LANG_NORWEGIAN: ::WORD = 0x14;
pub const LANG_OCCITAN: ::WORD = 0x82;
pub const LANG_ODIA: ::WORD = 0x48;
pub const LANG_ORIYA: ::WORD = 0x48;
pub const LANG_PASHTO: ::WORD = 0x63;
pub const LANG_PERSIAN: ::WORD = 0x29;
pub const LANG_POLISH: ::WORD = 0x15;
pub const LANG_PORTUGUESE: ::WORD = 0x16;
pub const LANG_PULAR: ::WORD = 0x67;
pub const LANG_PUNJABI: ::WORD = 0x46;
pub const LANG_QUECHUA: ::WORD = 0x6b;
pub const LANG_ROMANIAN: ::WORD = 0x18;
pub const LANG_ROMANSH: ::WORD = 0x17;
pub const LANG_RUSSIAN: ::WORD = 0x19;
pub const LANG_SAKHA: ::WORD = 0x85;
pub const LANG_SAMI: ::WORD = 0x3b;
pub const LANG_SANSKRIT: ::WORD = 0x4f;
pub const LANG_SCOTTISH_GAELIC: ::WORD = 0x91;
pub const LANG_SERBIAN: ::WORD = 0x1a;
pub const LANG_SERBIAN_NEUTRAL: ::WORD = 0x7c1a;
pub const LANG_SINDHI: ::WORD = 0x59;
pub const LANG_SINHALESE: ::WORD = 0x5b;
pub const LANG_SLOVAK: ::WORD = 0x1b;
pub const LANG_SLOVENIAN: ::WORD = 0x24;
pub const LANG_SOTHO: ::WORD = 0x6c;
pub const LANG_SPANISH: ::WORD = 0x0a;
pub const LANG_SWAHILI: ::WORD = 0x41;
pub const LANG_SWEDISH: ::WORD = 0x1d;
pub const LANG_SYRIAC: ::WORD = 0x5a;
pub const LANG_TAJIK: ::WORD = 0x28;
pub const LANG_TAMAZIGHT: ::WORD = 0x5f;
pub const LANG_TAMIL: ::WORD = 0x49;
pub const LANG_TATAR: ::WORD = 0x44;
pub const LANG_TELUGU: ::WORD = 0x4a;
pub const LANG_THAI: ::WORD = 0x1e;
pub const LANG_TIBETAN: ::WORD = 0x51;
pub const LANG_TIGRIGNA: ::WORD = 0x73;
pub const LANG_TIGRINYA: ::WORD = 0x73;
pub const LANG_TSWANA: ::WORD = 0x32;
pub const LANG_TURKISH: ::WORD = 0x1f;
pub const LANG_TURKMEN: ::WORD = 0x42;
pub const LANG_UIGHUR: ::WORD = 0x80;
pub const LANG_UKRAINIAN: ::WORD = 0x22;
pub const LANG_UPPER_SORBIAN: ::WORD = 0x2e;
pub const LANG_URDU: ::WORD = 0x20;
pub const LANG_UZBEK: ::WORD = 0x43;
pub const LANG_VALENCIAN: ::WORD = 0x03;
pub const LANG_VIETNAMESE: ::WORD = 0x2a;
pub const LANG_WELSH: ::WORD = 0x52;
pub const LANG_WOLOF: ::WORD = 0x88;
pub const LANG_XHOSA: ::WORD = 0x34;
pub const LANG_YAKUT: ::WORD = 0x85;
pub const LANG_YI: ::WORD = 0x78;
pub const LANG_YORUBA: ::WORD = 0x6a;
pub const LANG_ZULU: ::WORD = 0x35;
//1651
pub const SUBLANG_NEUTRAL: ::WORD = 0x00;
pub const SUBLANG_DEFAULT: ::WORD = 0x01;
pub const SUBLANG_SYS_DEFAULT: ::WORD = 0x02;
pub const SUBLANG_CUSTOM_DEFAULT: ::WORD = 0x03;
pub const SUBLANG_CUSTOM_UNSPECIFIED: ::WORD = 0x04;
pub const SUBLANG_UI_CUSTOM_DEFAULT: ::WORD = 0x05;
pub const SUBLANG_AFRIKAANS_SOUTH_AFRICA: ::WORD = 0x01;
pub const SUBLANG_ALBANIAN_ALBANIA: ::WORD = 0x01;
pub const SUBLANG_ALSATIAN_FRANCE: ::WORD = 0x01;
pub const SUBLANG_AMHARIC_ETHIOPIA: ::WORD = 0x01;
pub const SUBLANG_ARABIC_SAUDI_ARABIA: ::WORD = 0x01;
pub const SUBLANG_ARABIC_IRAQ: ::WORD = 0x02;
pub const SUBLANG_ARABIC_EGYPT: ::WORD = 0x03;
pub const SUBLANG_ARABIC_LIBYA: ::WORD = 0x04;
pub const SUBLANG_ARABIC_ALGERIA: ::WORD = 0x05;
pub const SUBLANG_ARABIC_MOROCCO: ::WORD = 0x06;
pub const SUBLANG_ARABIC_TUNISIA: ::WORD = 0x07;
pub const SUBLANG_ARABIC_OMAN: ::WORD = 0x08;
pub const SUBLANG_ARABIC_YEMEN: ::WORD = 0x09;
pub const SUBLANG_ARABIC_SYRIA: ::WORD = 0x0a;
pub const SUBLANG_ARABIC_JORDAN: ::WORD = 0x0b;
pub const SUBLANG_ARABIC_LEBANON: ::WORD = 0x0c;
pub const SUBLANG_ARABIC_KUWAIT: ::WORD = 0x0d;
pub const SUBLANG_ARABIC_UAE: ::WORD = 0x0e;
pub const SUBLANG_ARABIC_BAHRAIN: ::WORD = 0x0f;
pub const SUBLANG_ARABIC_QATAR: ::WORD = 0x10;
pub const SUBLANG_ARMENIAN_ARMENIA: ::WORD = 0x01;
pub const SUBLANG_ASSAMESE_INDIA: ::WORD = 0x01;
pub const SUBLANG_AZERI_LATIN: ::WORD = 0x01;
pub const SUBLANG_AZERI_CYRILLIC: ::WORD = 0x02;
pub const SUBLANG_AZERBAIJANI_AZERBAIJAN_LATIN: ::WORD = 0x01;
pub const SUBLANG_AZERBAIJANI_AZERBAIJAN_CYRILLIC: ::WORD = 0x02;
pub const SUBLANG_BANGLA_INDIA: ::WORD = 0x01;
pub const SUBLANG_BANGLA_BANGLADESH: ::WORD = 0x02;
pub const SUBLANG_BASHKIR_RUSSIA: ::WORD = 0x01;
pub const SUBLANG_BASQUE_BASQUE: ::WORD = 0x01;
pub const SUBLANG_BELARUSIAN_BELARUS: ::WORD = 0x01;
pub const SUBLANG_BENGALI_INDIA: ::WORD = 0x01;
pub const SUBLANG_BENGALI_BANGLADESH: ::WORD = 0x02;
pub const SUBLANG_BOSNIAN_BOSNIA_HERZEGOVINA_LATIN: ::WORD = 0x05;
pub const SUBLANG_BOSNIAN_BOSNIA_HERZEGOVINA_CYRILLIC: ::WORD = 0x08;
pub const SUBLANG_BRETON_FRANCE: ::WORD = 0x01;
pub const SUBLANG_BULGARIAN_BULGARIA: ::WORD = 0x01;
pub const SUBLANG_CATALAN_CATALAN: ::WORD = 0x01;
pub const SUBLANG_CENTRAL_KURDISH_IRAQ: ::WORD = 0x01;
pub const SUBLANG_CHEROKEE_CHEROKEE: ::WORD = 0x01;
pub const SUBLANG_CHINESE_TRADITIONAL: ::WORD = 0x01;
pub const SUBLANG_CHINESE_SIMPLIFIED: ::WORD = 0x02;
pub const SUBLANG_CHINESE_HONGKONG: ::WORD = 0x03;
pub const SUBLANG_CHINESE_SINGAPORE: ::WORD = 0x04;
pub const SUBLANG_CHINESE_MACAU: ::WORD = 0x05;
pub const SUBLANG_CORSICAN_FRANCE: ::WORD = 0x01;
pub const SUBLANG_CZECH_CZECH_REPUBLIC: ::WORD = 0x01;
pub const SUBLANG_CROATIAN_CROATIA: ::WORD = 0x01;
pub const SUBLANG_CROATIAN_BOSNIA_HERZEGOVINA_LATIN: ::WORD = 0x04;
pub const SUBLANG_DANISH_DENMARK: ::WORD = 0x01;
pub const SUBLANG_DARI_AFGHANISTAN: ::WORD = 0x01;
pub const SUBLANG_DIVEHI_MALDIVES: ::WORD = 0x01;
pub const SUBLANG_DUTCH: ::WORD = 0x01;
pub const SUBLANG_DUTCH_BELGIAN: ::WORD = 0x02;
pub const SUBLANG_ENGLISH_US: ::WORD = 0x01;
pub const SUBLANG_ENGLISH_UK: ::WORD = 0x02;
pub const SUBLANG_ENGLISH_AUS: ::WORD = 0x03;
pub const SUBLANG_ENGLISH_CAN: ::WORD = 0x04;
pub const SUBLANG_ENGLISH_NZ: ::WORD = 0x05;
pub const SUBLANG_ENGLISH_EIRE: ::WORD = 0x06;
pub const SUBLANG_ENGLISH_SOUTH_AFRICA: ::WORD = 0x07;
pub const SUBLANG_ENGLISH_JAMAICA: ::WORD = 0x08;
pub const SUBLANG_ENGLISH_CARIBBEAN: ::WORD = 0x09;
pub const SUBLANG_ENGLISH_BELIZE: ::WORD = 0x0a;
pub const SUBLANG_ENGLISH_TRINIDAD: ::WORD = 0x0b;
pub const SUBLANG_ENGLISH_ZIMBABWE: ::WORD = 0x0c;
pub const SUBLANG_ENGLISH_PHILIPPINES: ::WORD = 0x0d;
pub const SUBLANG_ENGLISH_INDIA: ::WORD = 0x10;
pub const SUBLANG_ENGLISH_MALAYSIA: ::WORD = 0x11;
pub const SUBLANG_ENGLISH_SINGAPORE: ::WORD = 0x12;
pub const SUBLANG_ESTONIAN_ESTONIA: ::WORD = 0x01;
pub const SUBLANG_FAEROESE_FAROE_ISLANDS: ::WORD = 0x01;
pub const SUBLANG_FILIPINO_PHILIPPINES: ::WORD = 0x01;
pub const SUBLANG_FINNISH_FINLAND: ::WORD = 0x01;
pub const SUBLANG_FRENCH: ::WORD = 0x01;
pub const SUBLANG_FRENCH_BELGIAN: ::WORD = 0x02;
pub const SUBLANG_FRENCH_CANADIAN: ::WORD = 0x03;
pub const SUBLANG_FRENCH_SWISS: ::WORD = 0x04;
pub const SUBLANG_FRENCH_LUXEMBOURG: ::WORD = 0x05;
pub const SUBLANG_FRENCH_MONACO: ::WORD = 0x06;
pub const SUBLANG_FRISIAN_NETHERLANDS: ::WORD = 0x01;
pub const SUBLANG_FULAH_SENEGAL: ::WORD = 0x02;
pub const SUBLANG_GALICIAN_GALICIAN: ::WORD = 0x01;
pub const SUBLANG_GEORGIAN_GEORGIA: ::WORD = 0x01;
pub const SUBLANG_GERMAN: ::WORD = 0x01;
pub const SUBLANG_GERMAN_SWISS: ::WORD = 0x02;
pub const SUBLANG_GERMAN_AUSTRIAN: ::WORD = 0x03;
pub const SUBLANG_GERMAN_LUXEMBOURG: ::WORD = 0x04;
pub const SUBLANG_GERMAN_LIECHTENSTEIN: ::WORD = 0x05;
pub const SUBLANG_GREEK_GREECE: ::WORD = 0x01;
pub const SUBLANG_GREENLANDIC_GREENLAND: ::WORD = 0x01;
pub const SUBLANG_GUJARATI_INDIA: ::WORD = 0x01;
pub const SUBLANG_HAUSA_NIGERIA_LATIN: ::WORD = 0x01;
pub const SUBLANG_HAWAIIAN_US: ::WORD = 0x01;
pub const SUBLANG_HEBREW_ISRAEL: ::WORD = 0x01;
pub const SUBLANG_HINDI_INDIA: ::WORD = 0x01;
pub const SUBLANG_HUNGARIAN_HUNGARY: ::WORD = 0x01;
pub const SUBLANG_ICELANDIC_ICELAND: ::WORD = 0x01;
pub const SUBLANG_IGBO_NIGERIA: ::WORD = 0x01;
pub const SUBLANG_INDONESIAN_INDONESIA: ::WORD = 0x01;
pub const SUBLANG_INUKTITUT_CANADA: ::WORD = 0x01;
pub const SUBLANG_INUKTITUT_CANADA_LATIN: ::WORD = 0x02;
pub const SUBLANG_IRISH_IRELAND: ::WORD = 0x02;
pub const SUBLANG_ITALIAN: ::WORD = 0x01;
pub const SUBLANG_ITALIAN_SWISS: ::WORD = 0x02;
pub const SUBLANG_JAPANESE_JAPAN: ::WORD = 0x01;
pub const SUBLANG_KANNADA_INDIA: ::WORD = 0x01;
pub const SUBLANG_KASHMIRI_SASIA: ::WORD = 0x02;
pub const SUBLANG_KASHMIRI_INDIA: ::WORD = 0x02;
pub const SUBLANG_KAZAK_KAZAKHSTAN: ::WORD = 0x01;
pub const SUBLANG_KHMER_CAMBODIA: ::WORD = 0x01;
pub const SUBLANG_KICHE_GUATEMALA: ::WORD = 0x01;
pub const SUBLANG_KINYARWANDA_RWANDA: ::WORD = 0x01;
pub const SUBLANG_KONKANI_INDIA: ::WORD = 0x01;
pub const SUBLANG_KOREAN: ::WORD = 0x01;
pub const SUBLANG_KYRGYZ_KYRGYZSTAN: ::WORD = 0x01;
pub const SUBLANG_LAO_LAO: ::WORD = 0x01;
pub const SUBLANG_LATVIAN_LATVIA: ::WORD = 0x01;
pub const SUBLANG_LITHUANIAN: ::WORD = 0x01;
pub const SUBLANG_LOWER_SORBIAN_GERMANY: ::WORD = 0x02;
pub const SUBLANG_LUXEMBOURGISH_LUXEMBOURG: ::WORD = 0x01;
pub const SUBLANG_MACEDONIAN_MACEDONIA: ::WORD = 0x01;
pub const SUBLANG_MALAY_MALAYSIA: ::WORD = 0x01;
pub const SUBLANG_MALAY_BRUNEI_DARUSSALAM: ::WORD = 0x02;
pub const SUBLANG_MALAYALAM_INDIA: ::WORD = 0x01;
pub const SUBLANG_MALTESE_MALTA: ::WORD = 0x01;
pub const SUBLANG_MAORI_NEW_ZEALAND: ::WORD = 0x01;
pub const SUBLANG_MAPUDUNGUN_CHILE: ::WORD = 0x01;
pub const SUBLANG_MARATHI_INDIA: ::WORD = 0x01;
pub const SUBLANG_MOHAWK_MOHAWK: ::WORD = 0x01;
pub const SUBLANG_MONGOLIAN_CYRILLIC_MONGOLIA: ::WORD = 0x01;
pub const SUBLANG_MONGOLIAN_PRC: ::WORD = 0x02;
pub const SUBLANG_NEPALI_INDIA: ::WORD = 0x02;
pub const SUBLANG_NEPALI_NEPAL: ::WORD = 0x01;
pub const SUBLANG_NORWEGIAN_BOKMAL: ::WORD = 0x01;
pub const SUBLANG_NORWEGIAN_NYNORSK: ::WORD = 0x02;
pub const SUBLANG_OCCITAN_FRANCE: ::WORD = 0x01;
pub const SUBLANG_ODIA_INDIA: ::WORD = 0x01;
pub const SUBLANG_ORIYA_INDIA: ::WORD = 0x01;
pub const SUBLANG_PASHTO_AFGHANISTAN: ::WORD = 0x01;
pub const SUBLANG_PERSIAN_IRAN: ::WORD = 0x01;
pub const SUBLANG_POLISH_POLAND: ::WORD = 0x01;
pub const SUBLANG_PORTUGUESE: ::WORD = 0x02;
pub const SUBLANG_PORTUGUESE_BRAZILIAN: ::WORD = 0x01;
pub const SUBLANG_PULAR_SENEGAL: ::WORD = 0x02;
pub const SUBLANG_PUNJABI_INDIA: ::WORD = 0x01;
pub const SUBLANG_PUNJABI_PAKISTAN: ::WORD = 0x02;
pub const SUBLANG_QUECHUA_BOLIVIA: ::WORD = 0x01;
pub const SUBLANG_QUECHUA_ECUADOR: ::WORD = 0x02;
pub const SUBLANG_QUECHUA_PERU: ::WORD = 0x03;
pub const SUBLANG_ROMANIAN_ROMANIA: ::WORD = 0x01;
pub const SUBLANG_ROMANSH_SWITZERLAND: ::WORD = 0x01;
pub const SUBLANG_RUSSIAN_RUSSIA: ::WORD = 0x01;
pub const SUBLANG_SAKHA_RUSSIA: ::WORD = 0x01;
pub const SUBLANG_SAMI_NORTHERN_NORWAY: ::WORD = 0x01;
pub const SUBLANG_SAMI_NORTHERN_SWEDEN: ::WORD = 0x02;
pub const SUBLANG_SAMI_NORTHERN_FINLAND: ::WORD = 0x03;
pub const SUBLANG_SAMI_LULE_NORWAY: ::WORD = 0x04;
pub const SUBLANG_SAMI_LULE_SWEDEN: ::WORD = 0x05;
pub const SUBLANG_SAMI_SOUTHERN_NORWAY: ::WORD = 0x06;
pub const SUBLANG_SAMI_SOUTHERN_SWEDEN: ::WORD = 0x07;
pub const SUBLANG_SAMI_SKOLT_FINLAND: ::WORD = 0x08;
pub const SUBLANG_SAMI_INARI_FINLAND: ::WORD = 0x09;
pub const SUBLANG_SANSKRIT_INDIA: ::WORD = 0x01;
pub const SUBLANG_SCOTTISH_GAELIC: ::WORD = 0x01;
pub const SUBLANG_SERBIAN_BOSNIA_HERZEGOVINA_LATIN: ::WORD = 0x06;
pub const SUBLANG_SERBIAN_BOSNIA_HERZEGOVINA_CYRILLIC: ::WORD = 0x07;
pub const SUBLANG_SERBIAN_MONTENEGRO_LATIN: ::WORD = 0x0b;
pub const SUBLANG_SERBIAN_MONTENEGRO_CYRILLIC: ::WORD = 0x0c;
pub const SUBLANG_SERBIAN_SERBIA_LATIN: ::WORD = 0x09;
pub const SUBLANG_SERBIAN_SERBIA_CYRILLIC: ::WORD = 0x0a;
pub const SUBLANG_SERBIAN_CROATIA: ::WORD = 0x01;
pub const SUBLANG_SERBIAN_LATIN: ::WORD = 0x02;
pub const SUBLANG_SERBIAN_CYRILLIC: ::WORD = 0x03;
pub const SUBLANG_SINDHI_INDIA: ::WORD = 0x01;
pub const SUBLANG_SINDHI_PAKISTAN: ::WORD = 0x02;
pub const SUBLANG_SINDHI_AFGHANISTAN: ::WORD = 0x02;
pub const SUBLANG_SINHALESE_SRI_LANKA: ::WORD = 0x01;
pub const SUBLANG_SOTHO_NORTHERN_SOUTH_AFRICA: ::WORD = 0x01;
pub const SUBLANG_SLOVAK_SLOVAKIA: ::WORD = 0x01;
pub const SUBLANG_SLOVENIAN_SLOVENIA: ::WORD = 0x01;
pub const SUBLANG_SPANISH: ::WORD = 0x01;
pub const SUBLANG_SPANISH_MEXICAN: ::WORD = 0x02;
pub const SUBLANG_SPANISH_MODERN: ::WORD = 0x03;
pub const SUBLANG_SPANISH_GUATEMALA: ::WORD = 0x04;
pub const SUBLANG_SPANISH_COSTA_RICA: ::WORD = 0x05;
pub const SUBLANG_SPANISH_PANAMA: ::WORD = 0x06;
pub const SUBLANG_SPANISH_DOMINICAN_REPUBLIC: ::WORD = 0x07;
pub const SUBLANG_SPANISH_VENEZUELA: ::WORD = 0x08;
pub const SUBLANG_SPANISH_COLOMBIA: ::WORD = 0x09;
pub const SUBLANG_SPANISH_PERU: ::WORD = 0x0a;
pub const SUBLANG_SPANISH_ARGENTINA: ::WORD = 0x0b;
pub const SUBLANG_SPANISH_ECUADOR: ::WORD = 0x0c;
pub const SUBLANG_SPANISH_CHILE: ::WORD = 0x0d;
pub const SUBLANG_SPANISH_URUGUAY: ::WORD = 0x0e;
pub const SUBLANG_SPANISH_PARAGUAY: ::WORD = 0x0f;
pub const SUBLANG_SPANISH_BOLIVIA: ::WORD = 0x10;
pub const SUBLANG_SPANISH_EL_SALVADOR: ::WORD = 0x11;
pub const SUBLANG_SPANISH_HONDURAS: ::WORD = 0x12;
pub const SUBLANG_SPANISH_NICARAGUA: ::WORD = 0x13;
pub const SUBLANG_SPANISH_PUERTO_RICO: ::WORD = 0x14;
pub const SUBLANG_SPANISH_US: ::WORD = 0x15;
pub const SUBLANG_SWAHILI_KENYA: ::WORD = 0x01;
pub const SUBLANG_SWEDISH: ::WORD = 0x01;
pub const SUBLANG_SWEDISH_FINLAND: ::WORD = 0x02;
pub const SUBLANG_SYRIAC_SYRIA: ::WORD = 0x01;
pub const SUBLANG_TAJIK_TAJIKISTAN: ::WORD = 0x01;
pub const SUBLANG_TAMAZIGHT_ALGERIA_LATIN: ::WORD = 0x02;
pub const SUBLANG_TAMAZIGHT_MOROCCO_TIFINAGH: ::WORD = 0x04;
pub const SUBLANG_TAMIL_INDIA: ::WORD = 0x01;
pub const SUBLANG_TAMIL_SRI_LANKA: ::WORD = 0x02;
pub const SUBLANG_TATAR_RUSSIA: ::WORD = 0x01;
pub const SUBLANG_TELUGU_INDIA: ::WORD = 0x01;
pub const SUBLANG_THAI_THAILAND: ::WORD = 0x01;
pub const SUBLANG_TIBETAN_PRC: ::WORD = 0x01;
pub const SUBLANG_TIGRIGNA_ERITREA: ::WORD = 0x02;
pub const SUBLANG_TIGRINYA_ERITREA: ::WORD = 0x02;
pub const SUBLANG_TIGRINYA_ETHIOPIA: ::WORD = 0x01;
pub const SUBLANG_TSWANA_BOTSWANA: ::WORD = 0x02;
pub const SUBLANG_TSWANA_SOUTH_AFRICA: ::WORD = 0x01;
pub const SUBLANG_TURKISH_TURKEY: ::WORD = 0x01;
pub const SUBLANG_TURKMEN_TURKMENISTAN: ::WORD = 0x01;
pub const SUBLANG_UIGHUR_PRC: ::WORD = 0x01;
pub const SUBLANG_UKRAINIAN_UKRAINE: ::WORD = 0x01;
pub const SUBLANG_UPPER_SORBIAN_GERMANY: ::WORD = 0x01;
pub const SUBLANG_URDU_PAKISTAN: ::WORD = 0x01;
pub const SUBLANG_URDU_INDIA: ::WORD = 0x02;
pub const SUBLANG_UZBEK_LATIN: ::WORD = 0x01;
pub const SUBLANG_UZBEK_CYRILLIC: ::WORD = 0x02;
pub const SUBLANG_VALENCIAN_VALENCIA: ::WORD = 0x02;
pub const SUBLANG_VIETNAMESE_VIETNAM: ::WORD = 0x01;
pub const SUBLANG_WELSH_UNITED_KINGDOM: ::WORD = 0x01;
pub const SUBLANG_WOLOF_SENEGAL: ::WORD = 0x01;
pub const SUBLANG_XHOSA_SOUTH_AFRICA: ::WORD = 0x01;
pub const SUBLANG_YAKUT_RUSSIA: ::WORD = 0x01;
pub const SUBLANG_YI_PRC: ::WORD = 0x01;
pub const SUBLANG_YORUBA_NIGERIA: ::WORD = 0x01;
pub const SUBLANG_ZULU_SOUTH_AFRICA: ::WORD = 0x01;
//1962
// FIXME: Once feature(const_fn) or some CTFE alternative becomes stable, MAKELANGID! can go
// unless we want to #[macro_export] it ...
macro_rules! MAKELANGID { ($p:expr, $s:expr) => ($s << 10 | $p) }
pub fn MAKELANGID(p: ::WORD, s: ::WORD) -> ::LANGID { MAKELANGID!(p, s) }
pub fn PRIMARYLANGID(lgid: ::LANGID) -> ::WORD { lgid & 0x3ff }
pub fn SUBLANGID(lgid: ::LANGID) -> ::WORD { lgid >> 10 }
//2019
pub const LANG_SYSTEM_DEFAULT: LANGID = MAKELANGID!(LANG_NEUTRAL, SUBLANG_SYS_DEFAULT);
pub const LANG_USER_DEFAULT: LANGID = MAKELANGID!(LANG_NEUTRAL, SUBLANG_DEFAULT);
//2214
pub const MAXIMUM_WAIT_OBJECTS: ::DWORD = 64;
pub const MAXIMUM_SUSPEND_COUNT: ::CHAR = MAXCHAR;
//2277
pub type KSPIN_LOCK = ::ULONG_PTR;
pub type PKSPIN_LOCK = *mut KSPIN_LOCK;
STRUCT!{struct M128A { // FIXME align 16
    Low: ULONGLONG,
    High: LONGLONG,
}}
pub type PM128A = *mut M128A;
#[cfg(target_arch = "x86")]
STRUCT!{nodebug struct XSAVE_FORMAT { // FIXME align 16
    ControlWord: ::WORD,
    StatusWord: ::WORD,
    TagWord: ::BYTE,
    Reserved1: ::BYTE,
    ErrorOpcode: ::WORD,
    ErrorOffset: ::DWORD,
    ErrorSelector: ::WORD,
    Reserved2: ::WORD,
    DataOffset: ::DWORD,
    DataSelector: ::WORD,
    Reserved3: ::WORD,
    MxCsr: ::DWORD,
    MxCsr_Mask: ::DWORD,
    FloatRegisters: [M128A; 8],
    XmmRegisters: [M128A; 8],
    Reserved4: [::BYTE; 224],
}}
#[cfg(target_arch = "x86_64")]
STRUCT!{nodebug struct XSAVE_FORMAT { // FIXME align 16
    ControlWord: ::WORD,
    StatusWord: ::WORD,
    TagWord: ::BYTE,
    Reserved1: ::BYTE,
    ErrorOpcode: ::WORD,
    ErrorOffset: ::DWORD,
    ErrorSelector: ::WORD,
    Reserved2: ::WORD,
    DataOffset: ::DWORD,
    DataSelector: ::WORD,
    Reserved3: ::WORD,
    MxCsr: ::DWORD,
    MxCsr_Mask: ::DWORD,
    FloatRegisters: [M128A; 8],
    XmmRegisters: [M128A; 16],
    Reserved4: [::BYTE; 96],
}}
//3563
#[cfg(target_arch = "x86")]
pub const SIZE_OF_80387_REGISTERS: usize = 80;
#[cfg(target_arch = "x86")]
STRUCT!{nodebug struct FLOATING_SAVE_AREA {
    ControlWord: ::DWORD,
    StatusWord: ::DWORD,
    TagWord: ::DWORD,
    ErrorOffset: ::DWORD,
    ErrorSelector: ::DWORD,
    DataOffset: ::DWORD,
    DataSelector: ::DWORD,
    RegisterArea: [::BYTE; SIZE_OF_80387_REGISTERS],
    Spare0: ::DWORD,
}}
#[cfg(target_arch = "x86")]
pub type PFLOATING_SAVE_AREA = *mut FLOATING_SAVE_AREA;
#[cfg(target_arch = "x86")]
pub const MAXIMUM_SUPPORTED_EXTENSION: usize = 512;
#[cfg(target_arch = "x86")]
STRUCT!{nodebug struct CONTEXT {
    ContextFlags: ::DWORD,
    Dr0: ::DWORD,
    Dr1: ::DWORD,
    Dr2: ::DWORD,
    Dr3: ::DWORD,
    Dr6: ::DWORD,
    Dr7: ::DWORD,
    FloatSave: FLOATING_SAVE_AREA,
    SegGs: ::DWORD,
    SegFs: ::DWORD,
    SegEs: ::DWORD,
    SegDs: ::DWORD,
    Edi: ::DWORD,
    Esi: ::DWORD,
    Ebx: ::DWORD,
    Edx: ::DWORD,
    Ecx: ::DWORD,
    Eax: ::DWORD,
    Ebp: ::DWORD,
    Eip: ::DWORD,
    SegCs: ::DWORD,
    EFlags: ::DWORD,
    Esp: ::DWORD,
    SegSs: ::DWORD,
    ExtendedRegisters: [::BYTE; MAXIMUM_SUPPORTED_EXTENSION],
}}
#[cfg(target_arch = "x86_64")]
pub type XMM_SAVE_AREA32 = XSAVE_FORMAT;
pub type PXMM_SAVE_AREA32 = *mut XSAVE_FORMAT;
// FIXME - Align 16
#[cfg(target_arch = "x86_64")]
STRUCT!{nodebug struct CONTEXT {
    P1Home: ::DWORD64,
    P2Home: ::DWORD64,
    P3Home: ::DWORD64,
    P4Home: ::DWORD64,
    P5Home: ::DWORD64,
    P6Home: ::DWORD64,
    ContextFlags: ::DWORD,
    MxCsr: ::DWORD,
    SegCs: ::WORD,
    SegDs: ::WORD,
    SegEs: ::WORD,
    SegFs: ::WORD,
    SegGs: ::WORD,
    SegSs: ::WORD,
    EFlags: ::DWORD,
    Dr0: ::DWORD64,
    Dr1: ::DWORD64,
    Dr2: ::DWORD64,
    Dr3: ::DWORD64,
    Dr6: ::DWORD64,
    Dr7: ::DWORD64,
    Rax: ::DWORD64,
    Rcx: ::DWORD64,
    Rdx: ::DWORD64,
    Rbx: ::DWORD64,
    Rsp: ::DWORD64,
    Rbp: ::DWORD64,
    Rsi: ::DWORD64,
    Rdi: ::DWORD64,
    R8: ::DWORD64,
    R9: ::DWORD64,
    R10: ::DWORD64,
    R11: ::DWORD64,
    R12: ::DWORD64,
    R13: ::DWORD64,
    R14: ::DWORD64,
    R15: ::DWORD64,
    Rip: ::DWORD64,
    FltSave: XMM_SAVE_AREA32,
    VectorRegister: [::M128A; 26],
    VectorControl: ::DWORD64,
    DebugControl: ::DWORD64,
    LastBranchToRip: ::DWORD64,
    LastBranchFromRip: ::DWORD64,
    LastExceptionToRip: ::DWORD64,
    LastExceptionFromRip: ::DWORD64,
}}
pub type PCONTEXT = *mut CONTEXT;
#[test]
fn test_CONTEXT_size() {
    use std::mem::size_of;
    if cfg!(target_arch = "x86_64") {
        assert_eq!(size_of::<CONTEXT>(), 1232)
    } else if cfg!(target_arch = "x86") {
        assert_eq!(size_of::<CONTEXT>(), 716)
    }
}
pub type RUNTIME_FUNCTION = IMAGE_RUNTIME_FUNCTION_ENTRY;
pub type PRUNTIME_FUNCTION = *mut RUNTIME_FUNCTION;
pub const UNWIND_HISTORY_TABLE_SIZE: usize = 12;
STRUCT!{struct UNWIND_HISTORY_TABLE_ENTRY {
    ImageBase: ::DWORD64,
    FunctionEntry: PRUNTIME_FUNCTION,
}}
pub type PUNWIND_HISTORY_TABLE_ENTRY = *mut UNWIND_HISTORY_TABLE_ENTRY;
STRUCT!{struct UNWIND_HISTORY_TABLE {
    Count: ::DWORD,
    LocalHint: ::BYTE,
    GlobalHint: ::BYTE,
    Search: ::BYTE,
    Once: ::BYTE,
    LowAddress: ::DWORD64,
    HighAddress: ::DWORD64,
    Entry: [UNWIND_HISTORY_TABLE_ENTRY; UNWIND_HISTORY_TABLE_SIZE],
}}
pub type PUNWIND_HISTORY_TABLE = *mut UNWIND_HISTORY_TABLE;
pub type PGET_RUNTIME_FUNCTION_CALLBACK = Option<unsafe extern "system" fn(
    ControlPc: ::DWORD64, Context: ::PVOID,
) -> PRUNTIME_FUNCTION>;
STRUCT!{struct KNONVOLATILE_CONTEXT_POINTERS_u1 {
    Xmm0: PM128A,
    Xmm1: PM128A,
    Xmm2: PM128A,
    Xmm3: PM128A,
    Xmm4: PM128A,
    Xmm5: PM128A,
    Xmm6: PM128A,
    Xmm7: PM128A,
    Xmm8: PM128A,
    Xmm9: PM128A,
    Xmm10: PM128A,
    Xmm11: PM128A,
    Xmm12: PM128A,
    Xmm14: PM128A,
    Xmm15: PM128A,
}}
STRUCT!{struct KNONVOLATILE_CONTEXT_POINTERS_u2 {
    Rax: ::DWORD64,
    Rcx: ::DWORD64,
    Rdx: ::DWORD64,
    Rbx: ::DWORD64,
    Rsp: ::DWORD64,
    Rbp: ::DWORD64,
    Rsi: ::DWORD64,
    Rdi: ::DWORD64,
    R8: ::DWORD64,
    R9: ::DWORD64,
    R10: ::DWORD64,
    R11: ::DWORD64,
    R12: ::DWORD64,
    R13: ::DWORD64,
    R14: ::DWORD64,
    R15: ::DWORD64,
}}
STRUCT!{struct KNONVOLATILE_CONTEXT_POINTERS {
    FloatingContext: [PM128A; 16],
    IntegerContext: [::PDWORD64; 16],
}}
// FIXME: all unions are untagged
UNION!(
    KNONVOLATILE_CONTEXT_POINTERS, FloatingContext, Xmms, Xmms_mut,
    KNONVOLATILE_CONTEXT_POINTERS_u1
);
UNION!(
    KNONVOLATILE_CONTEXT_POINTERS, IntegerContext, Regs, Regs_mut,
    KNONVOLATILE_CONTEXT_POINTERS_u2
);
pub type PKNONVOLATILE_CONTEXT_POINTERS = *mut KNONVOLATILE_CONTEXT_POINTERS;
//8983
pub const EXCEPTION_MAXIMUM_PARAMETERS: usize = 15;
STRUCT!{struct EXCEPTION_RECORD {
    ExceptionCode: ::DWORD,
    ExceptionFlags: ::DWORD,
    ExceptionRecord: *mut EXCEPTION_RECORD,
    ExceptionAddress: ::PVOID,
    NumberParameters: ::DWORD,
    ExceptionInformation: [::ULONG_PTR; EXCEPTION_MAXIMUM_PARAMETERS],
}}
pub type PEXCEPTION_RECORD = *mut EXCEPTION_RECORD;
//9023
STRUCT!{struct EXCEPTION_POINTERS {
    ExceptionRecord: PEXCEPTION_RECORD,
    ContextRecord: PCONTEXT,
}}
pub type PEXCEPTION_POINTERS = *mut EXCEPTION_POINTERS;
pub type PACCESS_TOKEN = ::PVOID;
pub type PSECURITY_DESCRIPTOR = ::PVOID;
pub type PSID = ::PVOID;
pub type PCLAIMS_BLOB = ::PVOID;
//9091
pub type ACCESS_MASK = ::DWORD;
pub type PACCESS_MASK = *mut ACCESS_MASK;
pub const DELETE: ::DWORD = 0x00010000;
pub const READ_CONTROL: ::DWORD = 0x00020000;
pub const WRITE_DAC: ::DWORD = 0x00040000;
pub const WRITE_OWNER: ::DWORD = 0x00080000;
pub const SYNCHRONIZE: ::DWORD = 0x00100000;
pub const STANDARD_RIGHTS_REQUIRED: ::DWORD = 0x000F0000;
pub const STANDARD_RIGHTS_READ: ::DWORD = READ_CONTROL;
pub const STANDARD_RIGHTS_WRITE: ::DWORD = READ_CONTROL;
pub const STANDARD_RIGHTS_EXECUTE: ::DWORD = READ_CONTROL;
pub const STANDARD_RIGHTS_ALL: ::DWORD = 0x001F0000;
pub const SPECIFIC_RIGHTS_ALL: ::DWORD = 0x0000FFFF;
pub const ACCESS_SYSTEM_SECURITY: ::DWORD = 0x01000000;
pub const MAXIMUM_ALLOWED: ::DWORD = 0x02000000;
pub const GENERIC_READ: ::DWORD = 0x80000000;
pub const GENERIC_WRITE: ::DWORD = 0x40000000;
pub const GENERIC_EXECUTE: ::DWORD = 0x20000000;
pub const GENERIC_ALL: ::DWORD = 0x10000000;
//9170
STRUCT!{struct LUID_AND_ATTRIBUTES {
    Luid: LUID,
    Attributes: ::DWORD,
}}
pub type PLUID_AND_ATTRIBUTES = *mut LUID_AND_ATTRIBUTES;
//9243
ENUM!{enum SID_NAME_USE {
    SidTypeUser = 1,
    SidTypeGroup,
    SidTypeDomain,
    SidTypeAlias,
    SidTypeWellKnownGroup,
    SidTypeDeletedAccount,
    SidTypeInvalid,
    SidTypeUnknown,
    SidTypeComputer,
    SidTypeLabel,
}}
pub type PSID_NAME_USE = *mut SID_NAME_USE;
STRUCT!{struct SID_AND_ATTRIBUTES {
    Sid: PSID,
    Attributes: ::DWORD,
}}
pub type PSID_AND_ATTRIBUTES = *mut SID_AND_ATTRIBUTES;
//9802
pub const ACL_REVISION: ::BYTE = 2;
pub const ACL_REVISION_DS: ::BYTE = 4;
pub const ACL_REVISION1: ::BYTE = 1;
pub const MIN_ACL_REVISION: ::BYTE = ACL_REVISION2;
pub const ACL_REVISION2: ::BYTE = 2;
pub const ACL_REVISION3: ::BYTE = 3;
pub const ACL_REVISION4: ::BYTE = 4;
pub const MAX_ACL_REVISION: ::BYTE = ACL_REVISION4;
STRUCT!{struct ACL {
    AclRevision: ::BYTE,
    Sbz1: ::BYTE,
    AclSize: ::WORD,
    AceCount: ::WORD,
    Sbz2: ::WORD,
}}
pub type PACL = *mut ACL;
//9888
pub const SE_PRIVILEGE_ENABLED_BY_DEFAULT: ::DWORD = 0x00000001;
pub const SE_PRIVILEGE_ENABLED: ::DWORD = 0x00000002;
pub const SE_PRIVILEGE_REMOVED: ::DWORD = 0x00000004;
pub const SE_PRIVILEGE_USED_FOR_ACCESS: ::DWORD = 0x80000000;
pub const SE_PRIVILEGE_VALID_ATTRIBUTES: ::DWORD = SE_PRIVILEGE_ENABLED_BY_DEFAULT | SE_PRIVILEGE_ENABLED | SE_PRIVILEGE_REMOVED | SE_PRIVILEGE_USED_FOR_ACCESS;
pub const PRIVILEGE_SET_ALL_NECESSARY: ::DWORD = 1;
//10689
pub const TOKEN_ASSIGN_PRIMARY: ::DWORD = 0x0001;
pub const TOKEN_DUPLICATE: ::DWORD = 0x0002;
pub const TOKEN_IMPERSONATE: ::DWORD = 0x0004;
pub const TOKEN_QUERY: ::DWORD = 0x0008;
pub const TOKEN_QUERY_SOURCE: ::DWORD = 0x0010;
pub const TOKEN_ADJUST_PRIVILEGES: ::DWORD = 0x0020;
pub const TOKEN_ADJUST_GROUPS: ::DWORD = 0x0040;
pub const TOKEN_ADJUST_DEFAULT: ::DWORD = 0x0080;
pub const TOKEN_ADJUST_SESSIONID: ::DWORD = 0x0100;
pub const TOKEN_ALL_ACCESS_P: ::DWORD = STANDARD_RIGHTS_REQUIRED | TOKEN_ASSIGN_PRIMARY
    | TOKEN_DUPLICATE | TOKEN_IMPERSONATE | TOKEN_QUERY | TOKEN_QUERY_SOURCE
    | TOKEN_ADJUST_PRIVILEGES | TOKEN_ADJUST_GROUPS | TOKEN_ADJUST_DEFAULT;
pub const TOKEN_ALL_ACCESS: ::DWORD = TOKEN_ALL_ACCESS_P | TOKEN_ADJUST_SESSIONID;
pub const TOKEN_READ: ::DWORD = STANDARD_RIGHTS_READ | TOKEN_QUERY;
pub const TOKEN_WRITE: ::DWORD = STANDARD_RIGHTS_WRITE | TOKEN_ADJUST_PRIVILEGES
    | TOKEN_ADJUST_GROUPS | TOKEN_ADJUST_DEFAULT;
pub const TOKEN_EXECUTE: ::DWORD = STANDARD_RIGHTS_EXECUTE;
//10823
STRUCT!{nodebug struct TOKEN_PRIVILEGES {
    PrivilegeCount: ::DWORD,
    Privileges: [LUID_AND_ATTRIBUTES; 0],
}}
pub type PTOKEN_PRIVILEGES = *mut TOKEN_PRIVILEGES;
//10965
pub const CLAIM_SECURITY_ATTRIBUTE_TYPE_INVALID: ::WORD = 0x00;
pub const CLAIM_SECURITY_ATTRIBUTE_TYPE_INT64: ::WORD = 0x01;
pub const CLAIM_SECURITY_ATTRIBUTE_TYPE_UINT64: ::WORD = 0x02;
pub const CLAIM_SECURITY_ATTRIBUTE_TYPE_STRING: ::WORD = 0x03;
STRUCT!{struct CLAIM_SECURITY_ATTRIBUTE_FQBN_VALUE {
    Version: ::DWORD64,
    Name: ::PWSTR,
}}
pub type PCLAIM_SECURITY_ATTRIBUTE_FQBN_VALUE = *mut CLAIM_SECURITY_ATTRIBUTE_FQBN_VALUE;
pub const CLAIM_SECURITY_ATTRIBUTE_TYPE_FQBN: ::WORD = 0x04;
pub const CLAIM_SECURITY_ATTRIBUTE_TYPE_SID: ::WORD = 0x05;
pub const CLAIM_SECURITY_ATTRIBUTE_TYPE_BOOLEAN: ::WORD = 0x06;
STRUCT!{struct CLAIM_SECURITY_ATTRIBUTE_OCTET_STRING_VALUE {
    pValue: ::PVOID,
    ValueLength: ::DWORD,
}}
pub type PCLAIM_SECURITY_ATTRIBUTE_OCTET_STRING_VALUE =
    *mut CLAIM_SECURITY_ATTRIBUTE_OCTET_STRING_VALUE;
pub const CLAIM_SECURITY_ATTRIBUTE_TYPE_OCTET_STRING: ::WORD = 0x10;
pub const CLAIM_SECURITY_ATTRIBUTE_NON_INHERITABLE: ::DWORD = 0x0001;
pub const CLAIM_SECURITY_ATTRIBUTE_VALUE_CASE_SENSITIVE: ::DWORD = 0x0002;
pub const CLAIM_SECURITY_ATTRIBUTE_USE_FOR_DENY_ONLY: ::DWORD = 0x0004;
pub const CLAIM_SECURITY_ATTRIBUTE_DISABLED_BY_DEFAULT: ::DWORD = 0x0008;
pub const CLAIM_SECURITY_ATTRIBUTE_DISABLED: ::DWORD = 0x0010;
pub const CLAIM_SECURITY_ATTRIBUTE_MANDATORY: ::DWORD = 0x0020;
pub const CLAIM_SECURITY_ATTRIBUTE_VALID_FLAGS: ::DWORD = CLAIM_SECURITY_ATTRIBUTE_NON_INHERITABLE
    | CLAIM_SECURITY_ATTRIBUTE_VALUE_CASE_SENSITIVE | CLAIM_SECURITY_ATTRIBUTE_USE_FOR_DENY_ONLY
    | CLAIM_SECURITY_ATTRIBUTE_DISABLED_BY_DEFAULT | CLAIM_SECURITY_ATTRIBUTE_DISABLED
    | CLAIM_SECURITY_ATTRIBUTE_MANDATORY;
pub const CLAIM_SECURITY_ATTRIBUTE_CUSTOM_FLAGS: ::DWORD = 0xFFFF0000;
STRUCT!{struct CLAIM_SECURITY_ATTRIBUTE_V1 {
    Name: ::PWSTR,
    ValueType: ::WORD,
    Reserved: ::WORD,
    Flags: ::DWORD,
    ValueCount: ::DWORD,
    // Put data here
}}
pub type PCLAIM_SECURITY_ATTRIBUTE_V1 = *mut CLAIM_SECURITY_ATTRIBUTE_V1;
STRUCT!{struct CLAIM_SECURITY_ATTRIBUTE_RELATIVE_V1 {
    Name: ::DWORD,
    ValueType: ::WORD,
    Reserved: ::WORD,
    Flags: ::DWORD,
    ValueCount: ::DWORD,
    // Put array here
}}
pub type PCLAIM_SECURITY_ATTRIBUTE_RELATIVE_V1 = *mut CLAIM_SECURITY_ATTRIBUTE_RELATIVE_V1;
pub const CLAIM_SECURITY_ATTRIBUTES_INFORMATION_VERSION_V1: ::WORD = 1;
pub const CLAIM_SECURITY_ATTRIBUTES_INFORMATION_VERSION: ::WORD =
    CLAIM_SECURITY_ATTRIBUTES_INFORMATION_VERSION_V1;
STRUCT!{struct CLAIM_SECURITY_ATTRIBUTES_INFORMATION {
    Version: ::WORD,
    Reserved: ::WORD,
    AttributeCount: ::DWORD,
    pAttributeV1: PCLAIM_SECURITY_ATTRIBUTE_V1,
}}
pub type PCLAIM_SECURITY_ATTRIBUTES_INFORMATION = *mut CLAIM_SECURITY_ATTRIBUTES_INFORMATION;
//11257
pub type SECURITY_INFORMATION = ::DWORD;
pub type PSECURITY_INFORMATION = *mut ::DWORD;
pub const OWNER_SECURITY_INFORMATION: SECURITY_INFORMATION = 0x00000001;
pub const GROUP_SECURITY_INFORMATION: SECURITY_INFORMATION = 0x00000002;
pub const DACL_SECURITY_INFORMATION: SECURITY_INFORMATION = 0x00000004;
pub const SACL_SECURITY_INFORMATION: SECURITY_INFORMATION = 0x00000008;
pub const LABEL_SECURITY_INFORMATION: SECURITY_INFORMATION = 0x00000010;
pub const ATTRIBUTE_SECURITY_INFORMATION: SECURITY_INFORMATION = 0x00000020;
pub const SCOPE_SECURITY_INFORMATION: SECURITY_INFORMATION = 0x00000040;
pub const PROCESS_TRUST_LABEL_SECURITY_INFORMATION: SECURITY_INFORMATION = 0x00000080;
pub const BACKUP_SECURITY_INFORMATION: SECURITY_INFORMATION = 0x00010000;
pub const PROTECTED_DACL_SECURITY_INFORMATION: SECURITY_INFORMATION = 0x80000000;
pub const PROTECTED_SACL_SECURITY_INFORMATION: SECURITY_INFORMATION = 0x40000000;
pub const UNPROTECTED_DACL_SECURITY_INFORMATION: SECURITY_INFORMATION = 0x20000000;
pub const UNPROTECTED_SACL_SECURITY_INFORMATION: SECURITY_INFORMATION = 0x10000000;
ENUM!{enum SE_LEARNING_MODE_DATA_TYPE {
    SeLearningModeInvalidType = 0,
    SeLearningModeSettings,
    SeLearningModeMax,
}}
STRUCT!{struct SECURITY_CAPABILITIES {
    AppContainerSid: PSID,
    Capabilities: PSID_AND_ATTRIBUTES,
    CapabilityCount: ::DWORD,
    Reserved: ::DWORD,
}}
pub type PSECURITY_CAPABILITIES = *mut SECURITY_CAPABILITIES;
pub type LPSECURITY_CAPABILITIES = *mut SECURITY_CAPABILITIES;
pub const PROCESS_TERMINATE: ::DWORD = 0x0001;
pub const PROCESS_CREATE_THREAD: ::DWORD = 0x0002;
pub const PROCESS_SET_SESSIONID: ::DWORD = 0x0004;
pub const PROCESS_VM_OPERATION: ::DWORD = 0x0008;
pub const PROCESS_VM_READ: ::DWORD = 0x0010;
pub const PROCESS_VM_WRITE: ::DWORD = 0x0020;
pub const PROCESS_DUP_HANDLE: ::DWORD = 0x0040;
pub const PROCESS_CREATE_PROCESS: ::DWORD = 0x0080;
pub const PROCESS_SET_QUOTA: ::DWORD = 0x0100;
pub const PROCESS_SET_INFORMATION: ::DWORD = 0x0200;
pub const PROCESS_QUERY_INFORMATION: ::DWORD = 0x0400;
pub const PROCESS_SUSPEND_RESUME: ::DWORD = 0x0800;
pub const PROCESS_QUERY_LIMITED_INFORMATION: ::DWORD = 0x1000;
pub const PROCESS_SET_LIMITED_INFORMATION: ::DWORD = 0x2000;
pub const PROCESS_ALL_ACCESS: ::DWORD = STANDARD_RIGHTS_REQUIRED | SYNCHRONIZE | 0xFFFF;
//11007
pub const THREAD_BASE_PRIORITY_LOWRT: ::DWORD = 15;
pub const THREAD_BASE_PRIORITY_MAX: ::DWORD = 2;
pub const THREAD_BASE_PRIORITY_MIN: ::DWORD = -2i32 as ::DWORD;
pub const THREAD_BASE_PRIORITY_IDLE: ::DWORD = -15i32 as ::DWORD;
//11018
STRUCT!{struct QUOTA_LIMITS {
    PagedPoolLimit: ::SIZE_T,
    NonPagedPoolLimit: ::SIZE_T,
    MinimumWorkingSetSize: ::SIZE_T,
    MaximumWorkingSetSize: ::SIZE_T,
    PagefileLimit: ::SIZE_T,
    TimeLimit: ::LARGE_INTEGER,
}}
pub type PQUOTA_LIMITS = *mut QUOTA_LIMITS;
pub const QUOTA_LIMITS_HARDWS_MIN_ENABLE: ::DWORD = 0x00000001;
pub const QUOTA_LIMITS_HARDWS_MIN_DISABLE: ::DWORD = 0x00000002;
pub const QUOTA_LIMITS_HARDWS_MAX_ENABLE: ::DWORD = 0x00000004;
pub const QUOTA_LIMITS_HARDWS_MAX_DISABLE: ::DWORD = 0x00000008;
pub const QUOTA_LIMITS_USE_DEFAULT_LIMITS: ::DWORD = 0x00000010;
STRUCT!{struct RATE_QUOTA_LIMIT {
    RateData: ::DWORD,
    BitFields: ::DWORD,
}}
BITFIELD!(RATE_QUOTA_LIMIT BitFields: ::DWORD [
    RatePercent set_RatePercent[0..7],
    Reserved0 set_Reserved0[7..32],
]);
pub type PRATE_QUOTA_LIMIT = *mut RATE_QUOTA_LIMIT;
STRUCT!{struct QUOTA_LIMITS_EX {
    PagedPoolLimit: ::SIZE_T,
    NonPagedPoolLimit: ::SIZE_T,
    MinimumWorkingSetSize: ::SIZE_T,
    MaximumWorkingSetSize: ::SIZE_T,
    PagefileLimit: ::SIZE_T,
    TimeLimit: ::LARGE_INTEGER,
    WorkingSetLimit: ::SIZE_T,
    Reserved2: ::SIZE_T,
    Reserved3: ::SIZE_T,
    Reserved4: ::SIZE_T,
    Flags: ::DWORD,
    CpuRateLimit: RATE_QUOTA_LIMIT,
}}
pub type PQUOTA_LIMITS_EX = *mut QUOTA_LIMITS_EX;
STRUCT!{struct IO_COUNTERS {
    ReadOperationCount: ::ULONGLONG,
    WriteOperationCount: ::ULONGLONG,
    OtherOperationCount: ::ULONGLONG,
    ReadTransferCount: ::ULONGLONG,
    WriteTransferCount: ::ULONGLONG,
    OtherTransferCount: ::ULONGLONG,
}}
pub type PIO_COUNTERS = *mut IO_COUNTERS;
//11192
STRUCT!{struct JOBOBJECT_BASIC_LIMIT_INFORMATION {
    PerProcessUserTimeLimit: ::LARGE_INTEGER,
    PerJobUserTimeLimit: ::LARGE_INTEGER,
    LimitFlags: ::DWORD,
    MinimumWorkingSetSize: ::SIZE_T,
    MaximumWorkingSetSize: ::SIZE_T,
    ActiveProcessLimit: ::DWORD,
    Affinity: ::ULONG_PTR,
    PriorityClass: ::DWORD,
    SchedulingClass: ::DWORD,
}}
pub type PJOBOBJECT_BASIC_LIMIT_INFORMATION = *mut JOBOBJECT_BASIC_LIMIT_INFORMATION;
STRUCT!{struct JOBOBJECT_EXTENDED_LIMIT_INFORMATION {
    BasicLimitInformation: JOBOBJECT_BASIC_LIMIT_INFORMATION,
    IoInfo: IO_COUNTERS,
    ProcessMemoryLimit: ::SIZE_T,
    JobMemoryLimit: ::SIZE_T,
    PeakProcessMemoryUsed: ::SIZE_T,
    PeakJobMemoryUsed: ::SIZE_T,
}}
pub type PJOBOBJECT_EXTENDED_LIMIT_INFORMATION = *mut JOBOBJECT_EXTENDED_LIMIT_INFORMATION;
STRUCT!{struct JOBOBJECT_BASIC_PROCESS_ID_LIST {
    NumberOfAssignedProcesses: ::DWORD,
    NumberOfProcessIdsInList: ::DWORD,
    ProcessIdList: [::ULONG_PTR; 0],
}}
//11712
pub const JOB_OBJECT_TERMINATE_AT_END_OF_JOB: ::DWORD = 0;
pub const JOB_OBJECT_POST_AT_END_OF_JOB: ::DWORD = 1;
pub const JOB_OBJECT_MSG_END_OF_JOB_TIME: ::DWORD = 1;
pub const JOB_OBJECT_MSG_END_OF_PROCESS_TIME: ::DWORD = 2;
pub const JOB_OBJECT_MSG_ACTIVE_PROCESS_LIMIT: ::DWORD = 3;
pub const JOB_OBJECT_MSG_ACTIVE_PROCESS_ZERO: ::DWORD = 4;
pub const JOB_OBJECT_MSG_NEW_PROCESS: ::DWORD = 6;
pub const JOB_OBJECT_MSG_EXIT_PROCESS: ::DWORD = 7;
pub const JOB_OBJECT_MSG_ABNORMAL_EXIT_PROCESS: ::DWORD = 8;
pub const JOB_OBJECT_MSG_PROCESS_MEMORY_LIMIT: ::DWORD = 9;
pub const JOB_OBJECT_MSG_JOB_MEMORY_LIMIT: ::DWORD = 10;
pub const JOB_OBJECT_MSG_NOTIFICATION_LIMIT: ::DWORD = 11;
pub const JOB_OBJECT_MSG_JOB_CYCLE_TIME_LIMIT: ::DWORD = 12;
pub const JOB_OBJECT_MSG_MINIMUM: ::DWORD = 1;
pub const JOB_OBJECT_MSG_MAXIMUM: ::DWORD = 12;
pub const JOB_OBJECT_VALID_COMPLETION_FILTER: ::DWORD = ((1 << (JOB_OBJECT_MSG_MAXIMUM + 1)) - 1)
    - ((1 << JOB_OBJECT_MSG_MINIMUM) - 1);
pub const JOB_OBJECT_LIMIT_WORKINGSET: ::DWORD = 0x00000001;
pub const JOB_OBJECT_LIMIT_PROCESS_TIME: ::DWORD = 0x00000002;
pub const JOB_OBJECT_LIMIT_JOB_TIME: ::DWORD = 0x00000004;
pub const JOB_OBJECT_LIMIT_ACTIVE_PROCESS: ::DWORD = 0x00000008;
pub const JOB_OBJECT_LIMIT_AFFINITY: ::DWORD = 0x00000010;
pub const JOB_OBJECT_LIMIT_PRIORITY_CLASS: ::DWORD = 0x00000020;
pub const JOB_OBJECT_LIMIT_PRESERVE_JOB_TIME: ::DWORD = 0x00000040;
pub const JOB_OBJECT_LIMIT_SCHEDULING_CLASS: ::DWORD = 0x00000080;
pub const JOB_OBJECT_LIMIT_PROCESS_MEMORY: ::DWORD = 0x00000100;
pub const JOB_OBJECT_LIMIT_JOB_MEMORY: ::DWORD = 0x00000200;
pub const JOB_OBJECT_LIMIT_DIE_ON_UNHANDLED_EXCEPTION: ::DWORD = 0x00000400;
pub const JOB_OBJECT_LIMIT_BREAKAWAY_OK: ::DWORD = 0x00000800;
pub const JOB_OBJECT_LIMIT_SILENT_BREAKAWAY_OK: ::DWORD = 0x00001000;
pub const JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE: ::DWORD = 0x00002000;
pub const JOB_OBJECT_LIMIT_SUBSET_AFFINITY: ::DWORD = 0x00004000;
pub const JOB_OBJECT_LIMIT_JOB_READ_BYTES: ::DWORD = 0x00010000;
pub const JOB_OBJECT_LIMIT_JOB_WRITE_BYTES: ::DWORD = 0x00020000;
pub const JOB_OBJECT_LIMIT_RATE_CONTROL: ::DWORD = 0x00040000;
pub const JOB_OBJECT_LIMIT_RESERVED3: ::DWORD = 0x00008000;
pub const JOB_OBJECT_LIMIT_VALID_FLAGS: ::DWORD = 0x0007ffff;
pub const JOB_OBJECT_BASIC_LIMIT_VALID_FLAGS: ::DWORD = 0x000000ff;
pub const JOB_OBJECT_EXTENDED_LIMIT_VALID_FLAGS: ::DWORD = 0x00007fff;
pub const JOB_OBJECT_NOTIFICATION_LIMIT_VALID_FLAGS: ::DWORD = 0x00070204;
pub const JOB_OBJECT_RESERVED_LIMIT_VALID_FLAGS: ::DWORD = 0x0007ffff;
pub const JOB_OBJECT_UILIMIT_NONE: ::DWORD = 0x00000000;
pub const JOB_OBJECT_UILIMIT_HANDLES: ::DWORD = 0x00000001;
pub const JOB_OBJECT_UILIMIT_READCLIPBOARD: ::DWORD = 0x00000002;
pub const JOB_OBJECT_UILIMIT_WRITECLIPBOARD: ::DWORD = 0x00000004;
pub const JOB_OBJECT_UILIMIT_SYSTEMPARAMETERS: ::DWORD = 0x00000008;
pub const JOB_OBJECT_UILIMIT_DISPLAYSETTINGS: ::DWORD = 0x00000010;
pub const JOB_OBJECT_UILIMIT_GLOBALATOMS: ::DWORD = 0x00000020;
pub const JOB_OBJECT_UILIMIT_DESKTOP: ::DWORD = 0x00000040;
pub const JOB_OBJECT_UILIMIT_EXITWINDOWS: ::DWORD = 0x00000080;
pub const JOB_OBJECT_UILIMIT_ALL: ::DWORD = 0x000000FF;
pub const JOB_OBJECT_UI_VALID_FLAGS: ::DWORD = 0x000000FF;
pub const JOB_OBJECT_SECURITY_NO_ADMIN: ::DWORD = 0x00000001;
pub const JOB_OBJECT_SECURITY_RESTRICTED_TOKEN: ::DWORD = 0x00000002;
pub const JOB_OBJECT_SECURITY_ONLY_TOKEN: ::DWORD = 0x00000004;
pub const JOB_OBJECT_SECURITY_FILTER_TOKENS: ::DWORD = 0x00000008;
pub const JOB_OBJECT_SECURITY_VALID_FLAGS: ::DWORD = 0x0000000f;
pub const JOB_OBJECT_CPU_RATE_CONTROL_ENABLE: ::DWORD = 0x1;
pub const JOB_OBJECT_CPU_RATE_CONTROL_WEIGHT_BASED: ::DWORD = 0x2;
pub const JOB_OBJECT_CPU_RATE_CONTROL_HARD_CAP: ::DWORD = 0x4;
pub const JOB_OBJECT_CPU_RATE_CONTROL_NOTIFY: ::DWORD = 0x8;
pub const JOB_OBJECT_CPU_RATE_CONTROL_VALID_FLAGS: ::DWORD = 0xf;
ENUM!{enum JOBOBJECTINFOCLASS {
    JobObjectBasicAccountingInformation = 1,
    JobObjectBasicLimitInformation,
    JobObjectBasicProcessIdList,
    JobObjectBasicUIRestrictions,
    JobObjectSecurityLimitInformation,
    JobObjectEndOfJobTimeInformation,
    JobObjectAssociateCompletionPortInformation,
    JobObjectBasicAndIoAccountingInformation,
    JobObjectExtendedLimitInformation,
    JobObjectJobSetInformation,
    JobObjectGroupInformation,
    JobObjectNotificationLimitInformation,
    JobObjectLimitViolationInformation,
    JobObjectGroupInformationEx,
    JobObjectCpuRateControlInformation,
    JobObjectCompletionFilter,
    JobObjectCompletionCounter,
    JobObjectReserved1Information = 18,
    JobObjectReserved2Information,
    JobObjectReserved3Information,
    JobObjectReserved4Information,
    JobObjectReserved5Information,
    JobObjectReserved6Information,
    JobObjectReserved7Information,
    JobObjectReserved8Information,
    JobObjectReserved9Information,
    MaxJobObjectInfoClass,
}}
//12063
pub const SECTION_QUERY: ::DWORD = 0x0001;
pub const SECTION_MAP_WRITE: ::DWORD = 0x0002;
pub const SECTION_MAP_READ: ::DWORD = 0x0004;
pub const SECTION_MAP_EXECUTE: ::DWORD = 0x0008;
pub const SECTION_EXTEND_SIZE: ::DWORD = 0x0010;
pub const SECTION_MAP_EXECUTE_EXPLICIT: ::DWORD = 0x0020;
pub const SECTION_ALL_ACCESS: ::DWORD = STANDARD_RIGHTS_REQUIRED | SECTION_QUERY
    | SECTION_MAP_WRITE | SECTION_MAP_READ | SECTION_MAP_EXECUTE | SECTION_EXTEND_SIZE;
//12100
pub const PAGE_NOACCESS: ::DWORD = 0x01;
pub const PAGE_READONLY: ::DWORD = 0x02;
pub const PAGE_READWRITE: ::DWORD = 0x04;
pub const PAGE_WRITECOPY: ::DWORD = 0x08;
pub const PAGE_EXECUTE: ::DWORD = 0x10;
pub const PAGE_EXECUTE_READ: ::DWORD = 0x20;
pub const PAGE_EXECUTE_READWRITE: ::DWORD = 0x40;
pub const PAGE_EXECUTE_WRITECOPY: ::DWORD = 0x80;
pub const PAGE_GUARD: ::DWORD = 0x100;
pub const PAGE_NOCACHE: ::DWORD = 0x200;
pub const PAGE_WRITECOMBINE: ::DWORD = 0x400;
pub const PAGE_REVERT_TO_FILE_MAP: ::DWORD = 0x80000000;
pub const PAGE_TARGETS_NO_UPDATE: ::DWORD = 0x40000000;
pub const PAGE_TARGETS_INVALID: ::DWORD = 0x40000000;
pub const MEM_COMMIT: ::DWORD = 0x1000;
pub const MEM_RESERVE: ::DWORD = 0x2000;
pub const MEM_DECOMMIT: ::DWORD = 0x4000;
pub const MEM_RELEASE: ::DWORD = 0x8000;
pub const MEM_FREE: ::DWORD = 0x10000;
pub const MEM_PRIVATE: ::DWORD = 0x20000;
pub const MEM_MAPPED: ::DWORD = 0x40000;
pub const MEM_RESET: ::DWORD = 0x80000;
pub const MEM_TOP_DOWN: ::DWORD = 0x100000;
pub const MEM_WRITE_WATCH: ::DWORD = 0x200000;
pub const MEM_PHYSICAL: ::DWORD = 0x400000;
pub const MEM_ROTATE: ::DWORD = 0x800000;
pub const MEM_DIFFERENT_IMAGE_BASE_OK: ::DWORD = 0x800000;
pub const MEM_RESET_UNDO: ::DWORD = 0x1000000;
pub const MEM_LARGE_PAGES: ::DWORD = 0x20000000;
pub const MEM_4MB_PAGES: ::DWORD = 0x80000000;
pub const SEC_FILE: ::DWORD = 0x800000;
pub const SEC_IMAGE: ::DWORD = 0x1000000;
pub const SEC_PROTECTED_IMAGE: ::DWORD = 0x2000000;
pub const SEC_RESERVE: ::DWORD = 0x4000000;
pub const SEC_COMMIT: ::DWORD = 0x8000000;
pub const SEC_NOCACHE: ::DWORD = 0x10000000;
pub const SEC_WRITECOMBINE: ::DWORD = 0x40000000;
pub const SEC_LARGE_PAGES: ::DWORD = 0x80000000;
pub const SEC_IMAGE_NO_EXECUTE: ::DWORD = (SEC_IMAGE | SEC_NOCACHE);
pub const MEM_IMAGE: ::DWORD = SEC_IMAGE;
pub const WRITE_WATCH_FLAG_RESET: ::DWORD = 0x01;
pub const MEM_UNMAP_WITH_TRANSIENT_BOOST: ::DWORD = 0x01;
//12217
pub const FILE_READ_DATA: ::DWORD = 0x0001;
pub const FILE_LIST_DIRECTORY: ::DWORD = 0x0001;
pub const FILE_WRITE_DATA: ::DWORD = 0x0002;
pub const FILE_ADD_FILE: ::DWORD = 0x0002;
pub const FILE_APPEND_DATA: ::DWORD = 0x0004;
pub const FILE_ADD_SUBDIRECTORY: ::DWORD = 0x0004;
pub const FILE_CREATE_PIPE_INSTANCE: ::DWORD = 0x0004;
pub const FILE_READ_EA: ::DWORD = 0x0008;
pub const FILE_WRITE_EA: ::DWORD = 0x0010;
pub const FILE_EXECUTE: ::DWORD = 0x0020;
pub const FILE_TRAVERSE: ::DWORD = 0x0020;
pub const FILE_DELETE_CHILD: ::DWORD = 0x0040;
pub const FILE_READ_ATTRIBUTES: ::DWORD = 0x0080;
pub const FILE_WRITE_ATTRIBUTES: ::DWORD = 0x0100;
pub const FILE_ALL_ACCESS: ::DWORD = STANDARD_RIGHTS_REQUIRED | SYNCHRONIZE | 0x1FF;
pub const FILE_GENERIC_READ: ::DWORD = STANDARD_RIGHTS_READ | FILE_READ_DATA
    | FILE_READ_ATTRIBUTES | FILE_READ_EA | SYNCHRONIZE;
pub const FILE_GENERIC_WRITE: ::DWORD = STANDARD_RIGHTS_WRITE | FILE_WRITE_DATA
    | FILE_WRITE_ATTRIBUTES | FILE_WRITE_EA | FILE_APPEND_DATA | SYNCHRONIZE;
pub const FILE_GENERIC_EXECUTE: ::DWORD = STANDARD_RIGHTS_EXECUTE | FILE_READ_ATTRIBUTES
    | FILE_EXECUTE | SYNCHRONIZE;
pub const FILE_SHARE_READ: ::DWORD = 0x00000001;
pub const FILE_SHARE_WRITE: ::DWORD = 0x00000002;
pub const FILE_SHARE_DELETE: ::DWORD = 0x00000004;
pub const FILE_ATTRIBUTE_READONLY: ::DWORD = 0x00000001;
pub const FILE_ATTRIBUTE_HIDDEN: ::DWORD = 0x00000002;
pub const FILE_ATTRIBUTE_SYSTEM: ::DWORD = 0x00000004;
pub const FILE_ATTRIBUTE_DIRECTORY: ::DWORD = 0x00000010;
pub const FILE_ATTRIBUTE_ARCHIVE: ::DWORD = 0x00000020;
pub const FILE_ATTRIBUTE_DEVICE: ::DWORD = 0x00000040;
pub const FILE_ATTRIBUTE_NORMAL: ::DWORD = 0x00000080;
pub const FILE_ATTRIBUTE_TEMPORARY: ::DWORD = 0x00000100;
pub const FILE_ATTRIBUTE_SPARSE_FILE: ::DWORD = 0x00000200;
pub const FILE_ATTRIBUTE_REPARSE_POINT: ::DWORD = 0x00000400;
pub const FILE_ATTRIBUTE_COMPRESSED: ::DWORD = 0x00000800;
pub const FILE_ATTRIBUTE_OFFLINE: ::DWORD = 0x00001000;
pub const FILE_ATTRIBUTE_NOT_CONTENT_INDEXED: ::DWORD = 0x00002000;
pub const FILE_ATTRIBUTE_ENCRYPTED: ::DWORD = 0x00004000;
pub const FILE_ATTRIBUTE_INTEGRITY_STREAM: ::DWORD = 0x00008000;
pub const FILE_ATTRIBUTE_VIRTUAL: ::DWORD = 0x00010000;
pub const FILE_ATTRIBUTE_NO_SCRUB_DATA: ::DWORD = 0x00020000;
pub const FILE_ATTRIBUTE_EA: ::DWORD = 0x00040000;
pub const FILE_NOTIFY_CHANGE_FILE_NAME: ::DWORD = 0x00000001;
pub const FILE_NOTIFY_CHANGE_DIR_NAME: ::DWORD = 0x00000002;
pub const FILE_NOTIFY_CHANGE_ATTRIBUTES: ::DWORD = 0x00000004;
pub const FILE_NOTIFY_CHANGE_SIZE: ::DWORD = 0x00000008;
pub const FILE_NOTIFY_CHANGE_LAST_WRITE: ::DWORD = 0x00000010;
pub const FILE_NOTIFY_CHANGE_LAST_ACCESS: ::DWORD = 0x00000020;
pub const FILE_NOTIFY_CHANGE_CREATION: ::DWORD = 0x00000040;
pub const FILE_NOTIFY_CHANGE_SECURITY: ::DWORD = 0x00000100;
pub const FILE_ACTION_ADDED: ::DWORD = 0x00000001;
pub const FILE_ACTION_REMOVED: ::DWORD = 0x00000002;
pub const FILE_ACTION_MODIFIED: ::DWORD = 0x00000003;
pub const FILE_ACTION_RENAMED_OLD_NAME: ::DWORD = 0x00000004;
pub const FILE_ACTION_RENAMED_NEW_NAME: ::DWORD = 0x00000005;
pub const MAILSLOT_NO_MESSAGE: ::DWORD = 0xFFFFFFFF;
pub const MAILSLOT_WAIT_FOREVER: ::DWORD = 0xFFFFFFFF;
pub const FILE_CASE_SENSITIVE_SEARCH: ::DWORD = 0x00000001;
pub const FILE_CASE_PRESERVED_NAMES: ::DWORD = 0x00000002;
pub const FILE_UNICODE_ON_DISK: ::DWORD = 0x00000004;
pub const FILE_PERSISTENT_ACLS: ::DWORD = 0x00000008;
pub const FILE_FILE_COMPRESSION: ::DWORD = 0x00000010;
pub const FILE_VOLUME_QUOTAS: ::DWORD = 0x00000020;
pub const FILE_SUPPORTS_SPARSE_FILES: ::DWORD = 0x00000040;
pub const FILE_SUPPORTS_REPARSE_POINTS: ::DWORD = 0x00000080;
pub const FILE_SUPPORTS_REMOTE_STORAGE: ::DWORD = 0x00000100;
pub const FILE_VOLUME_IS_COMPRESSED: ::DWORD = 0x00008000;
pub const FILE_SUPPORTS_OBJECT_IDS: ::DWORD = 0x00010000;
pub const FILE_SUPPORTS_ENCRYPTION: ::DWORD = 0x00020000;
pub const FILE_NAMED_STREAMS: ::DWORD = 0x00040000;
pub const FILE_READ_ONLY_VOLUME: ::DWORD = 0x00080000;
pub const FILE_SEQUENTIAL_WRITE_ONCE: ::DWORD = 0x00100000;
pub const FILE_SUPPORTS_TRANSACTIONS: ::DWORD = 0x00200000;
pub const FILE_SUPPORTS_HARD_LINKS: ::DWORD = 0x00400000;
pub const FILE_SUPPORTS_EXTENDED_ATTRIBUTES: ::DWORD = 0x00800000;
pub const FILE_SUPPORTS_OPEN_BY_FILE_ID: ::DWORD = 0x01000000;
pub const FILE_SUPPORTS_USN_JOURNAL: ::DWORD = 0x02000000;
pub const FILE_SUPPORTS_INTEGRITY_STREAMS: ::DWORD = 0x04000000;
pub const FILE_INVALID_FILE_ID: ::LONGLONG = -1;
STRUCT!{struct FILE_ID_128 {
    Identifier: [::BYTE; 16],
}}
pub type PFILE_ID_128 = *mut FILE_ID_128;
STRUCT!{struct FILE_NOTIFY_INFORMATION {
    NextEntryOffset: ::DWORD,
    Action: ::DWORD,
    FileNameLength: ::DWORD,
    FileName: [::WCHAR; 0],
}}
STRUCT!{struct FILE_SEGMENT_ELEMENT {
    Buffer: ::PVOID64,
    Alignment: ::ULONGLONG,
}}
pub type PFILE_SEGMENT_ELEMENT = *mut FILE_SEGMENT_ELEMENT;
//12475
pub const IO_REPARSE_TAG_MOUNT_POINT: ::DWORD = 0xA0000003;
pub const IO_REPARSE_TAG_HSM: ::DWORD = 0xC0000004;
pub const IO_REPARSE_TAG_HSM2: ::DWORD = 0x80000006;
pub const IO_REPARSE_TAG_SIS: ::DWORD = 0x80000007;
pub const IO_REPARSE_TAG_WIM: ::DWORD = 0x80000008;
pub const IO_REPARSE_TAG_CSV: ::DWORD = 0x80000009;
pub const IO_REPARSE_TAG_DFS: ::DWORD = 0x8000000A;
pub const IO_REPARSE_TAG_SYMLINK: ::DWORD = 0xA000000C;
pub const IO_REPARSE_TAG_DFSR: ::DWORD = 0x80000012;
pub const IO_REPARSE_TAG_DEDUP: ::DWORD = 0x80000013;
pub const IO_REPARSE_TAG_NFS: ::DWORD = 0x80000014;
pub const IO_REPARSE_TAG_FILE_PLACEHOLDER: ::DWORD = 0x80000015;
pub const IO_REPARSE_TAG_WOF: ::DWORD = 0x80000017;
//12788
pub const DUPLICATE_CLOSE_SOURCE: ::DWORD = 0x00000001;
pub const DUPLICATE_SAME_ACCESS: ::DWORD = 0x00000002;
//14708
STRUCT!{struct PROCESSOR_POWER_POLICY_INFO {
    TimeCheck: ::DWORD,
    DemoteLimit: ::DWORD,
    PromoteLimit: ::DWORD,
    DemotePercent: ::BYTE,
    PromotePercent: ::BYTE,
    Spare: [::BYTE; 2],
    Reserved: ::DWORD,
}}
BITFIELD!(PROCESSOR_POWER_POLICY_INFO Reserved: ::DWORD [
    AllowDemotion set_AllowDemotion[0..1],
    AllowPromotion set_AllowPromotion[1..2],
]);
pub type PPROCESSOR_POWER_POLICY_INFO = *mut PROCESSOR_POWER_POLICY_INFO;
//15000
STRUCT!{struct IMAGE_FILE_HEADER {
    Machine: ::WORD,
    NumberOfSections: ::WORD,
    TimeDateStamp: ::DWORD,
    PointerToSymbolTable: ::DWORD,
    NumberOfSymbols: ::DWORD,
    SizeOfOptionalHeader: ::WORD,
    Characteristics: ::WORD,
}}
pub type PIMAGE_FILE_HEADER = *mut IMAGE_FILE_HEADER;
pub const IMAGE_SIZEOF_FILE_HEADER: usize = 20;
pub const IMAGE_FILE_RELOCS_STRIPPED: ::WORD = 0x0001;
pub const IMAGE_FILE_EXECUTABLE_IMAGE: ::WORD = 0x0002;
pub const IMAGE_FILE_LINE_NUMS_STRIPPED: ::WORD = 0x0004;
pub const IMAGE_FILE_LOCAL_SYMS_STRIPPED: ::WORD = 0x0008;
pub const IMAGE_FILE_AGGRESIVE_WS_TRIM: ::WORD = 0x0010;
pub const IMAGE_FILE_LARGE_ADDRESS_AWARE: ::WORD = 0x0020;
pub const IMAGE_FILE_BYTES_REVERSED_LO: ::WORD = 0x0080;
pub const IMAGE_FILE_32BIT_MACHINE: ::WORD = 0x0100;
pub const IMAGE_FILE_DEBUG_STRIPPED: ::WORD = 0x0200;
pub const IMAGE_FILE_REMOVABLE_RUN_FROM_SWAP: ::WORD = 0x0400;
pub const IMAGE_FILE_NET_RUN_FROM_SWAP: ::WORD = 0x0800;
pub const IMAGE_FILE_SYSTEM: ::WORD = 0x1000;
pub const IMAGE_FILE_DLL: ::WORD = 0x2000;
pub const IMAGE_FILE_UP_SYSTEM_ONLY: ::WORD = 0x4000;
pub const IMAGE_FILE_BYTES_REVERSED_HI: ::WORD = 0x8000;
pub const IMAGE_FILE_MACHINE_UNKNOWN: ::WORD = 0;
pub const IMAGE_FILE_MACHINE_I386: ::WORD = 0x014c;
pub const IMAGE_FILE_MACHINE_R3000: ::WORD = 0x0162;
pub const IMAGE_FILE_MACHINE_R4000: ::WORD = 0x0166;
pub const IMAGE_FILE_MACHINE_R10000: ::WORD = 0x0168;
pub const IMAGE_FILE_MACHINE_WCEMIPSV2: ::WORD = 0x0169;
pub const IMAGE_FILE_MACHINE_ALPHA: ::WORD = 0x0184;
pub const IMAGE_FILE_MACHINE_SH3: ::WORD = 0x01a2;
pub const IMAGE_FILE_MACHINE_SH3DSP: ::WORD = 0x01a3;
pub const IMAGE_FILE_MACHINE_SH3E: ::WORD = 0x01a4;
pub const IMAGE_FILE_MACHINE_SH4: ::WORD = 0x01a6;
pub const IMAGE_FILE_MACHINE_SH5: ::WORD = 0x01a8;
pub const IMAGE_FILE_MACHINE_ARM: ::WORD = 0x01c0;
pub const IMAGE_FILE_MACHINE_THUMB: ::WORD = 0x01c2;
pub const IMAGE_FILE_MACHINE_ARMNT: ::WORD = 0x01c4;
pub const IMAGE_FILE_MACHINE_AM33: ::WORD = 0x01d3;
pub const IMAGE_FILE_MACHINE_POWERPC: ::WORD = 0x01F0;
pub const IMAGE_FILE_MACHINE_POWERPCFP: ::WORD = 0x01f1;
pub const IMAGE_FILE_MACHINE_IA64: ::WORD = 0x0200;
pub const IMAGE_FILE_MACHINE_MIPS16: ::WORD = 0x0266;
pub const IMAGE_FILE_MACHINE_ALPHA64: ::WORD = 0x0284;
pub const IMAGE_FILE_MACHINE_MIPSFPU: ::WORD = 0x0366;
pub const IMAGE_FILE_MACHINE_MIPSFPU16: ::WORD = 0x0466;
pub const IMAGE_FILE_MACHINE_AXP64: ::WORD = IMAGE_FILE_MACHINE_ALPHA64;
pub const IMAGE_FILE_MACHINE_TRICORE: ::WORD = 0x0520;
pub const IMAGE_FILE_MACHINE_CEF: ::WORD = 0x0CEF;
pub const IMAGE_FILE_MACHINE_EBC: ::WORD = 0x0EBC;
pub const IMAGE_FILE_MACHINE_AMD64: ::WORD = 0x8664;
pub const IMAGE_FILE_MACHINE_M32R: ::WORD = 0x9041;
pub const IMAGE_FILE_MACHINE_CEE: ::WORD = 0xC0EE;
STRUCT!{struct IMAGE_DATA_DIRECTORY {
    VirtualAddress: ::DWORD,
    Size: ::DWORD,
}}
pub type PIMAGE_DATA_DIRECTORY = *mut IMAGE_DATA_DIRECTORY;
pub const IMAGE_NUMBEROF_DIRECTORY_ENTRIES: usize = 16;
STRUCT!{struct IMAGE_OPTIONAL_HEADER32 {
    Magic: ::WORD,
    MajorLinkerVersion: ::BYTE,
    MinorLinkerVersion: ::BYTE,
    SizeOfCode: ::DWORD,
    SizeOfInitializedData: ::DWORD,
    SizeOfUninitializedData: ::DWORD,
    AddressOfEntryPoint: ::DWORD,
    BaseOfCode: ::DWORD,
    BaseOfData: ::DWORD,
    ImageBase: ::DWORD,
    SectionAlignment: ::DWORD,
    FileAlignment: ::DWORD,
    MajorOperatingSystemVersion: ::WORD,
    MinorOperatingSystemVersion: ::WORD,
    MajorImageVersion: ::WORD,
    MinorImageVersion: ::WORD,
    MajorSubsystemVersion: ::WORD,
    MinorSubsystemVersion: ::WORD,
    Win32VersionValue: ::DWORD,
    SizeOfImage: ::DWORD,
    SizeOfHeaders: ::DWORD,
    CheckSum: ::DWORD,
    Subsystem: ::WORD,
    DllCharacteristics: ::WORD,
    SizeOfStackReserve: ::DWORD,
    SizeOfStackCommit: ::DWORD,
    SizeOfHeapReserve: ::DWORD,
    SizeOfHeapCommit: ::DWORD,
    LoaderFlags: ::DWORD,
    NumberOfRvaAndSizes: ::DWORD,
    DataDirectory: [IMAGE_DATA_DIRECTORY; IMAGE_NUMBEROF_DIRECTORY_ENTRIES],
}}
pub type PIMAGE_OPTIONAL_HEADER32 = *mut IMAGE_OPTIONAL_HEADER32;
STRUCT!{struct IMAGE_ROM_OPTIONAL_HEADER {
    Magic: ::WORD,
    MajorLinkerVersion: ::BYTE,
    MinorLinkerVersion: ::BYTE,
    SizeOfCode: ::DWORD,
    SizeOfInitializedData: ::DWORD,
    SizeOfUninitializedData: ::DWORD,
    AddressOfEntryPoint: ::DWORD,
    BaseOfCode: ::DWORD,
    BaseOfData: ::DWORD,
    BaseOfBss: ::DWORD,
    GprMask: ::DWORD,
    CprMask: [::DWORD; 4],
    GpValue: ::DWORD,
}}
pub type PIMAGE_ROM_OPTIONAL_HEADER = *mut IMAGE_ROM_OPTIONAL_HEADER;
STRUCT!{struct IMAGE_OPTIONAL_HEADER64 {
    Magic: ::WORD,
    MajorLinkerVersion: ::BYTE,
    MinorLinkerVersion: ::BYTE,
    SizeOfCode: ::DWORD,
    SizeOfInitializedData: ::DWORD,
    SizeOfUninitializedData: ::DWORD,
    AddressOfEntryPoint: ::DWORD,
    BaseOfCode: ::DWORD,
    ImageBase: ::ULONGLONG,
    SectionAlignment: ::DWORD,
    FileAlignment: ::DWORD,
    MajorOperatingSystemVersion: ::WORD,
    MinorOperatingSystemVersion: ::WORD,
    MajorImageVersion: ::WORD,
    MinorImageVersion: ::WORD,
    MajorSubsystemVersion: ::WORD,
    MinorSubsystemVersion: ::WORD,
    Win32VersionValue: ::DWORD,
    SizeOfImage: ::DWORD,
    SizeOfHeaders: ::DWORD,
    CheckSum: ::DWORD,
    Subsystem: ::WORD,
    DllCharacteristics: ::WORD,
    SizeOfStackReserve: ULONGLONG,
    SizeOfStackCommit: ULONGLONG,
    SizeOfHeapReserve: ULONGLONG,
    SizeOfHeapCommit: ULONGLONG,
    LoaderFlags: ::DWORD,
    NumberOfRvaAndSizes: ::DWORD,
    DataDirectory: [IMAGE_DATA_DIRECTORY; IMAGE_NUMBEROF_DIRECTORY_ENTRIES],
}}
pub type PIMAGE_OPTIONAL_HEADER64 = *mut IMAGE_OPTIONAL_HEADER64;
pub const IMAGE_NT_OPTIONAL_HDR32_MAGIC: ::WORD = 0x10b;
pub const IMAGE_NT_OPTIONAL_HDR64_MAGIC: ::WORD = 0x20b;
pub const IMAGE_ROM_OPTIONAL_HDR_MAGIC: ::WORD = 0x107;
#[cfg(target_arch = "x86_64")]
pub type IMAGE_OPTIONAL_HEADER = IMAGE_OPTIONAL_HEADER64;
#[cfg(target_arch = "x86_64")]
pub type PIMAGE_OPTIONAL_HEADER = PIMAGE_OPTIONAL_HEADER64;
#[cfg(target_arch = "x86")]
pub type IMAGE_OPTIONAL_HEADER = IMAGE_OPTIONAL_HEADER32;
#[cfg(target_arch = "x86")]
pub type PIMAGE_OPTIONAL_HEADER = PIMAGE_OPTIONAL_HEADER32;
STRUCT!{struct IMAGE_NT_HEADERS64 {
    Signature: ::DWORD,
    FileHeader: IMAGE_FILE_HEADER,
    OptionalHeader: IMAGE_OPTIONAL_HEADER64,
}}
pub type PIMAGE_NT_HEADERS64 = *mut IMAGE_NT_HEADERS64;
STRUCT!{struct IMAGE_NT_HEADERS32 {
    Signature: ::DWORD,
    FileHeader: IMAGE_FILE_HEADER,
    OptionalHeader: IMAGE_OPTIONAL_HEADER32,
}}
pub type PIMAGE_NT_HEADERS32 = *mut IMAGE_NT_HEADERS32;
STRUCT!{struct IMAGE_ROM_HEADERS {
    FileHeader: IMAGE_FILE_HEADER,
    OptionalHeader: IMAGE_ROM_OPTIONAL_HEADER,
}}
pub type PIMAGE_ROM_HEADERS = *mut IMAGE_ROM_HEADERS;
#[cfg(target_arch = "x86_64")]
pub type IMAGE_NT_HEADERS = IMAGE_NT_HEADERS64;
#[cfg(target_arch = "x86_64")]
pub type PIMAGE_NT_HEADERS = PIMAGE_NT_HEADERS64;
#[cfg(target_arch = "x86")]
pub type IMAGE_NT_HEADERS = IMAGE_NT_HEADERS32;
#[cfg(target_arch = "x86")]
pub type PIMAGE_NT_HEADERS = PIMAGE_NT_HEADERS32;
pub const IMAGE_SUBSYSTEM_UNKNOWN: ::WORD = 0;
pub const IMAGE_SUBSYSTEM_NATIVE: ::WORD = 1;
pub const IMAGE_SUBSYSTEM_WINDOWS_GUI: ::WORD = 2;
pub const IMAGE_SUBSYSTEM_WINDOWS_CUI: ::WORD = 3;
pub const IMAGE_SUBSYSTEM_OS2_CUI: ::WORD = 5;
pub const IMAGE_SUBSYSTEM_POSIX_CUI: ::WORD = 7;
pub const IMAGE_SUBSYSTEM_NATIVE_WINDOWS: ::WORD = 8;
pub const IMAGE_SUBSYSTEM_WINDOWS_CE_GUI: ::WORD = 9;
pub const IMAGE_SUBSYSTEM_EFI_APPLICATION: ::WORD = 10;
pub const IMAGE_SUBSYSTEM_EFI_BOOT_SERVICE_DRIVER: ::WORD = 11;
pub const IMAGE_SUBSYSTEM_EFI_RUNTIME_DRIVER: ::WORD = 12;
pub const IMAGE_SUBSYSTEM_EFI_ROM: ::WORD = 13;
pub const IMAGE_SUBSYSTEM_XBOX: ::WORD = 14;
pub const IMAGE_SUBSYSTEM_WINDOWS_BOOT_APPLICATION: ::WORD = 16;
pub const IMAGE_DLLCHARACTERISTICS_HIGH_ENTROPY_VA: ::WORD = 0x0020;
pub const IMAGE_DLLCHARACTERISTICS_DYNAMIC_BASE: ::WORD = 0x0040;
pub const IMAGE_DLLCHARACTERISTICS_FORCE_INTEGRITY: ::WORD = 0x0080;
pub const IMAGE_DLLCHARACTERISTICS_NX_COMPAT: ::WORD = 0x0100;
pub const IMAGE_DLLCHARACTERISTICS_NO_ISOLATION: ::WORD = 0x0200;
pub const IMAGE_DLLCHARACTERISTICS_NO_SEH: ::WORD = 0x0400;
pub const IMAGE_DLLCHARACTERISTICS_NO_BIND: ::WORD = 0x0800;
pub const IMAGE_DLLCHARACTERISTICS_APPCONTAINER: ::WORD = 0x1000;
pub const IMAGE_DLLCHARACTERISTICS_WDM_DRIVER: ::WORD = 0x2000;
pub const IMAGE_DLLCHARACTERISTICS_GUARD_CF: ::WORD = 0x4000;
pub const IMAGE_DLLCHARACTERISTICS_TERMINAL_SERVER_AWARE: ::WORD = 0x8000;
pub const IMAGE_DIRECTORY_ENTRY_EXPORT: ::WORD = 0;
pub const IMAGE_DIRECTORY_ENTRY_IMPORT: ::WORD = 1;
pub const IMAGE_DIRECTORY_ENTRY_RESOURCE: ::WORD = 2;
pub const IMAGE_DIRECTORY_ENTRY_EXCEPTION: ::WORD = 3;
pub const IMAGE_DIRECTORY_ENTRY_SECURITY: ::WORD = 4;
pub const IMAGE_DIRECTORY_ENTRY_BASERELOC: ::WORD = 5;
pub const IMAGE_DIRECTORY_ENTRY_DEBUG: ::WORD = 6;
pub const IMAGE_DIRECTORY_ENTRY_ARCHITECTURE: ::WORD = 7;
pub const IMAGE_DIRECTORY_ENTRY_GLOBALPTR: ::WORD = 8;
pub const IMAGE_DIRECTORY_ENTRY_TLS: ::WORD = 9;
pub const IMAGE_DIRECTORY_ENTRY_LOAD_CONFIG: ::WORD = 10;
pub const IMAGE_DIRECTORY_ENTRY_BOUND_IMPORT: ::WORD = 11;
pub const IMAGE_DIRECTORY_ENTRY_IAT: ::WORD = 12;
pub const IMAGE_DIRECTORY_ENTRY_DELAY_IMPORT: ::WORD = 13;
pub const IMAGE_DIRECTORY_ENTRY_COM_DESCRIPTOR: ::WORD = 14;
STRUCT!{struct ANON_OBJECT_HEADER {
    Sig1: ::WORD,
    Sig2: ::WORD,
    Version: ::WORD,
    Machine: ::WORD,
    TimeDateStamp: ::DWORD,
    ClassID: ::CLSID,
    SizeOfData: ::DWORD,
}}
STRUCT!{struct ANON_OBJECT_HEADER_V2 {
    Sig1: ::WORD,
    Sig2: ::WORD,
    Version: ::WORD,
    Machine: ::WORD,
    TimeDateStamp: ::DWORD,
    ClassID: ::CLSID,
    SizeOfData: ::DWORD,
    Flags: ::DWORD,
    MetaDataSize: ::DWORD,
    MetaDataOffset: ::DWORD,
}}
STRUCT!{struct ANON_OBJECT_HEADER_BIGOBJ {
    Sig1: ::WORD,
    Sig2: ::WORD,
    Version: ::WORD,
    Machine: ::WORD,
    TimeDateStamp: ::DWORD,
    ClassID: ::CLSID,
    SizeOfData: ::DWORD,
    Flags: ::DWORD,
    MetaDataSize: ::DWORD,
    MetaDataOffset: ::DWORD,
    NumberOfSections: ::DWORD,
    PointerToSymbolTable: ::DWORD,
    NumberOfSymbols: ::DWORD,
}}
pub const IMAGE_SIZEOF_SHORT_NAME: usize = 8;
STRUCT!{struct IMAGE_SECTION_HEADER {
    Name: [::BYTE; IMAGE_SIZEOF_SHORT_NAME],
    PhysicalAddressOrVirtualSize: ::DWORD,
    VirtualAddress: ::DWORD,
    SizeOfRawData: ::DWORD,
    PointerToRawData: ::DWORD,
    PointerToRelocations: ::DWORD,
    PointerToLinenumbers: ::DWORD,
    NumberOfRelocations: ::WORD,
    NumberOfLinenumbers: ::WORD,
    Characteristics: ::DWORD,
}}
pub type PIMAGE_SECTION_HEADER = *mut IMAGE_SECTION_HEADER;
pub const IMAGE_SIZEOF_SECTION_HEADER: usize = 40;
pub const IMAGE_SCN_TYPE_NO_PAD: ::DWORD = 0x00000008;
pub const IMAGE_SCN_CNT_CODE: ::DWORD = 0x00000020;
pub const IMAGE_SCN_CNT_INITIALIZED_DATA: ::DWORD = 0x00000040;
pub const IMAGE_SCN_CNT_UNINITIALIZED_DATA: ::DWORD = 0x00000080;
pub const IMAGE_SCN_LNK_OTHER: ::DWORD = 0x00000100;
pub const IMAGE_SCN_LNK_INFO: ::DWORD = 0x00000200;
pub const IMAGE_SCN_LNK_REMOVE: ::DWORD = 0x00000800;
pub const IMAGE_SCN_LNK_COMDAT: ::DWORD = 0x00001000;
pub const IMAGE_SCN_NO_DEFER_SPEC_EXC: ::DWORD = 0x00004000;
pub const IMAGE_SCN_GPREL: ::DWORD = 0x00008000;
pub const IMAGE_SCN_MEM_FARDATA: ::DWORD = 0x00008000;
pub const IMAGE_SCN_MEM_PURGEABLE: ::DWORD = 0x00020000;
pub const IMAGE_SCN_MEM_16BIT: ::DWORD = 0x00020000;
pub const IMAGE_SCN_MEM_LOCKED: ::DWORD = 0x00040000;
pub const IMAGE_SCN_MEM_PRELOAD: ::DWORD = 0x00080000;
pub const IMAGE_SCN_ALIGN_1BYTES: ::DWORD = 0x00100000;
pub const IMAGE_SCN_ALIGN_2BYTES: ::DWORD = 0x00200000;
pub const IMAGE_SCN_ALIGN_4BYTES: ::DWORD = 0x00300000;
pub const IMAGE_SCN_ALIGN_8BYTES: ::DWORD = 0x00400000;
pub const IMAGE_SCN_ALIGN_16BYTES: ::DWORD = 0x00500000;
pub const IMAGE_SCN_ALIGN_32BYTES: ::DWORD = 0x00600000;
pub const IMAGE_SCN_ALIGN_64BYTES: ::DWORD = 0x00700000;
pub const IMAGE_SCN_ALIGN_128BYTES: ::DWORD = 0x00800000;
pub const IMAGE_SCN_ALIGN_256BYTES: ::DWORD = 0x00900000;
pub const IMAGE_SCN_ALIGN_512BYTES: ::DWORD = 0x00A00000;
pub const IMAGE_SCN_ALIGN_1024BYTES: ::DWORD = 0x00B00000;
pub const IMAGE_SCN_ALIGN_2048BYTES: ::DWORD = 0x00C00000;
pub const IMAGE_SCN_ALIGN_4096BYTES: ::DWORD = 0x00D00000;
pub const IMAGE_SCN_ALIGN_8192BYTES: ::DWORD = 0x00E00000;
pub const IMAGE_SCN_ALIGN_MASK: ::DWORD = 0x00F00000;
pub const IMAGE_SCN_LNK_NRELOC_OVFL: ::DWORD = 0x01000000;
pub const IMAGE_SCN_MEM_DISCARDABLE: ::DWORD = 0x02000000;
pub const IMAGE_SCN_MEM_NOT_CACHED: ::DWORD = 0x04000000;
pub const IMAGE_SCN_MEM_NOT_PAGED: ::DWORD = 0x08000000;
pub const IMAGE_SCN_MEM_SHARED: ::DWORD = 0x10000000;
pub const IMAGE_SCN_MEM_EXECUTE: ::DWORD = 0x20000000;
pub const IMAGE_SCN_MEM_READ: ::DWORD = 0x40000000;
pub const IMAGE_SCN_MEM_WRITE: ::DWORD = 0x80000000;
pub const IMAGE_SCN_SCALE_INDEX: ::DWORD = 0x00000001;
//16590
STRUCT!{struct IMAGE_DEBUG_DIRECTORY {
    Characteristics: ::DWORD,
    TimeDateStamp: ::DWORD,
    MajorVersion: ::WORD,
    MinorVersion: ::WORD,
    Type: ::DWORD,
    SizeOfData: ::DWORD,
    AddressOfRawData: ::DWORD,
    PointerToRawData: ::DWORD,
}}
pub type PIMAGE_DEBUG_DIRECTORY = *mut IMAGE_DEBUG_DIRECTORY;
pub const IMAGE_DEBUG_TYPE_UNKNOWN: ::DWORD = 0;
pub const IMAGE_DEBUG_TYPE_COFF: ::DWORD = 1;
pub const IMAGE_DEBUG_TYPE_CODEVIEW: ::DWORD = 2;
pub const IMAGE_DEBUG_TYPE_FPO: ::DWORD = 3;
pub const IMAGE_DEBUG_TYPE_MISC: ::DWORD = 4;
pub const IMAGE_DEBUG_TYPE_EXCEPTION: ::DWORD = 5;
pub const IMAGE_DEBUG_TYPE_FIXUP: ::DWORD = 6;
pub const IMAGE_DEBUG_TYPE_OMAP_TO_SRC: ::DWORD = 7;
pub const IMAGE_DEBUG_TYPE_OMAP_FROM_SRC: ::DWORD = 8;
pub const IMAGE_DEBUG_TYPE_BORLAND: ::DWORD = 9;
pub const IMAGE_DEBUG_TYPE_RESERVED10: ::DWORD = 10;
pub const IMAGE_DEBUG_TYPE_CLSID: ::DWORD = 11;
STRUCT!{struct IMAGE_COFF_SYMBOLS_HEADER {
    NumberOfSymbols: ::DWORD,
    LvaToFirstSymbol: ::DWORD,
    NumberOfLinenumbers: ::DWORD,
    LvaToFirstLinenumber: ::DWORD,
    RvaToFirstByteOfCode: ::DWORD,
    RvaToLastByteOfCode: ::DWORD,
    RvaToFirstByteOfData: ::DWORD,
    RvaToLastByteOfData: ::DWORD,
}}
pub type PIMAGE_COFF_SYMBOLS_HEADER = *mut IMAGE_COFF_SYMBOLS_HEADER;
STRUCT!{struct IMAGE_RUNTIME_FUNCTION_ENTRY {
    BeginAddress: ::DWORD,
    EndAddress: ::DWORD,
    UnwindInfoAddress: ::DWORD,
}}
UNION!(IMAGE_RUNTIME_FUNCTION_ENTRY, UnwindInfoAddress, UnwindData, UnwindData_mut, ::DWORD);
pub type PIMAGE_RUNTIME_FUNCTION_ENTRY = *mut IMAGE_RUNTIME_FUNCTION_ENTRY;
pub const FRAME_FPO: ::WORD = 0;
pub const FRAME_TRAP: ::WORD = 1;
pub const FRAME_TSS: ::WORD = 2;
pub const FRAME_NONFPO: ::WORD = 3;
STRUCT!{struct FPO_DATA {
    ulOffStart: ::DWORD,
    cbProcSize: ::DWORD,
    cdwLocals: ::DWORD,
    cdwParams: ::WORD,
    bitfield: ::WORD,
}}
pub type PFPO_DATA = *mut FPO_DATA;
pub const SIZEOF_RFPO_DATA: usize = 16;
pub const IMAGE_DEBUG_MISC_EXENAME: ::DWORD = 1;
STRUCT!{struct IMAGE_DEBUG_MISC {
    DataType: ::DWORD,
    Length: ::DWORD,
    Unicode: ::BOOLEAN,
    Reserved: [::BYTE; 3],
    Data: [::BYTE; 0],
}}
pub type PIMAGE_DEBUG_MISC = *mut IMAGE_DEBUG_MISC;
STRUCT!{struct IMAGE_FUNCTION_ENTRY {
    StartingAddress: ::DWORD,
    EndingAddress: ::DWORD,
    EndOfPrologue: ::DWORD,
}}
pub type PIMAGE_FUNCTION_ENTRY = *mut IMAGE_FUNCTION_ENTRY;
STRUCT!{struct IMAGE_FUNCTION_ENTRY64 {
    StartingAddress: ::ULONGLONG,
    EndingAddress: ::ULONGLONG,
    EndOfPrologueOrUnwindInfoAddress: ::ULONGLONG,
}}
pub type PIMAGE_FUNCTION_ENTRY64 = *mut IMAGE_FUNCTION_ENTRY64;
//18245
pub const HEAP_NO_SERIALIZE: ::DWORD = 0x00000001;
pub const HEAP_GROWABLE: ::DWORD = 0x00000002;
pub const HEAP_GENERATE_EXCEPTIONS: ::DWORD = 0x00000004;
pub const HEAP_ZERO_MEMORY: ::DWORD = 0x00000008;
pub const HEAP_REALLOC_IN_PLACE_ONLY: ::DWORD = 0x00000010;
pub const HEAP_TAIL_CHECKING_ENABLED: ::DWORD = 0x00000020;
pub const HEAP_FREE_CHECKING_ENABLED: ::DWORD = 0x00000040;
pub const HEAP_DISABLE_COALESCE_ON_FREE: ::DWORD = 0x00000080;
pub const HEAP_CREATE_ALIGN_16: ::DWORD = 0x00010000;
pub const HEAP_CREATE_ENABLE_TRACING: ::DWORD = 0x00020000;
pub const HEAP_CREATE_ENABLE_EXECUTE: ::DWORD = 0x00040000;
pub const HEAP_MAXIMUM_TAG: ::DWORD = 0x0FFF;
pub const HEAP_PSEUDO_TAG_FLAG: ::DWORD = 0x8000;
pub const HEAP_TAG_SHIFT: ::DWORD = 18;
//18145
STRUCT!{struct RTL_CRITICAL_SECTION_DEBUG {
    Type: ::WORD,
    CreatorBackTraceIndex: ::WORD,
    CriticalSection: *mut ::RTL_CRITICAL_SECTION,
    ProcessLocksList: ::LIST_ENTRY,
    EntryCount: ::DWORD,
    ContentionCount: ::DWORD,
    Flags: ::DWORD,
    CreatorBackTraceIndexHigh: ::WORD,
    SpareWORD: ::WORD,
}}
pub type PRTL_CRITICAL_SECTION_DEBUG = *mut RTL_CRITICAL_SECTION_DEBUG;
pub type RTL_RESOURCE_DEBUG = RTL_CRITICAL_SECTION_DEBUG;
pub type PRTL_RESOURCE_DEBUG = *mut RTL_CRITICAL_SECTION_DEBUG;
pub const RTL_CRITSECT_TYPE: ::WORD = 0;
pub const RTL_RESOURCE_TYPE: ::WORD = 1;
pub const RTL_CRITICAL_SECTION_FLAG_NO_DEBUG_INFO: ::ULONG_PTR = 0x01000000;
pub const RTL_CRITICAL_SECTION_FLAG_DYNAMIC_SPIN: ::ULONG_PTR = 0x02000000;
pub const RTL_CRITICAL_SECTION_FLAG_STATIC_INIT: ::ULONG_PTR = 0x04000000;
pub const RTL_CRITICAL_SECTION_FLAG_RESOURCE_TYPE: ::ULONG_PTR = 0x08000000;
pub const RTL_CRITICAL_SECTION_FLAG_FORCE_DEBUG_INFO: ::ULONG_PTR = 0x10000000;
pub const RTL_CRITICAL_SECTION_ALL_FLAG_BITS: ::ULONG_PTR = 0xFF000000;
pub const RTL_CRITICAL_SECTION_FLAG_RESERVED: ::ULONG_PTR = RTL_CRITICAL_SECTION_ALL_FLAG_BITS & !(RTL_CRITICAL_SECTION_FLAG_NO_DEBUG_INFO | RTL_CRITICAL_SECTION_FLAG_DYNAMIC_SPIN | RTL_CRITICAL_SECTION_FLAG_STATIC_INIT | RTL_CRITICAL_SECTION_FLAG_RESOURCE_TYPE | RTL_CRITICAL_SECTION_FLAG_FORCE_DEBUG_INFO);
pub const RTL_CRITICAL_SECTION_DEBUG_FLAG_STATIC_INIT: ::DWORD = 0x00000001;
STRUCT!{struct RTL_CRITICAL_SECTION {
    DebugInfo: ::PRTL_CRITICAL_SECTION_DEBUG,
    LockCount: ::LONG,
    RecursionCount: ::LONG,
    OwningThread: ::HANDLE,
    LockSemaphore: ::HANDLE,
    SpinCount: ::ULONG_PTR,
}}
pub type PRTL_CRITICAL_SECTION = *mut RTL_CRITICAL_SECTION;
STRUCT!{struct RTL_SRWLOCK {
    Ptr: ::PVOID,
}}
pub type PRTL_SRWLOCK = *mut RTL_SRWLOCK;
pub const RTL_SRWLOCK_INIT: RTL_SRWLOCK = RTL_SRWLOCK { Ptr: 0 as PVOID };
STRUCT!{struct RTL_CONDITION_VARIABLE {
    Ptr: ::PVOID,
}}
pub type PRTL_CONDITION_VARIABLE = *mut RTL_CONDITION_VARIABLE;
pub const RTL_CONDITION_VARIABLE_INIT: RTL_CONDITION_VARIABLE = RTL_CONDITION_VARIABLE {
    Ptr: 0 as PVOID
};
//18204
pub type PAPCFUNC = Option<unsafe extern "system" fn(Parameter: ::ULONG_PTR)>;
pub type PVECTORED_EXCEPTION_HANDLER = Option<unsafe extern "system" fn(
    ExceptionInfo: *mut EXCEPTION_POINTERS,
) -> ::LONG>;
ENUM!{enum HEAP_INFORMATION_CLASS {
    HeapCompatibilityInformation = 0,
    HeapEnableTerminationOnCorruption = 1,
    HeapOptimizeResources = 3,
}}
//pub use self::HEAP_INFORMATION_CLASS::*;
pub const HEAP_OPTIMIZE_RESOURCES_CURRENT_VERSION: ::DWORD = 1;
STRUCT!{struct HEAP_OPTIMIZE_RESOURCES_INFORMATION {
    Version: ::DWORD,
    Flags: ::DWORD,
}}
pub type PHEAP_OPTIMIZE_RESOURCES_INFORMATION = *mut HEAP_OPTIMIZE_RESOURCES_INFORMATION;
pub const WT_EXECUTEDEFAULT: ::ULONG = 0x00000000;
pub const WT_EXECUTEINIOTHREAD: ::ULONG = 0x00000001;
pub const WT_EXECUTEINUITHREAD: ::ULONG = 0x00000002;
pub const WT_EXECUTEINWAITTHREAD: ::ULONG = 0x00000004;
pub const WT_EXECUTEONLYONCE: ::ULONG = 0x00000008;
pub const WT_EXECUTEINTIMERTHREAD: ::ULONG = 0x00000020;
pub const WT_EXECUTELONGFUNCTION: ::ULONG = 0x00000010;
pub const WT_EXECUTEINPERSISTENTIOTHREAD: ::ULONG = 0x00000040;
pub const WT_EXECUTEINPERSISTENTTHREAD: ::ULONG = 0x00000080;
pub const WT_TRANSFER_IMPERSONATION: ::ULONG = 0x00000100;
pub type WAITORTIMERCALLBACKFUNC = Option<unsafe extern "system" fn(::PVOID, ::BOOLEAN)>;
pub type WORKERCALLBACKFUNC = Option<unsafe extern "system" fn(::PVOID)>;
pub type APC_CALLBACK_FUNCTION = Option<unsafe extern "system" fn(::DWORD, ::PVOID, ::PVOID)>;
pub type WAITORTIMERCALLBACK = WAITORTIMERCALLBACKFUNC;
pub type PFLS_CALLBACK_FUNCTION = Option<unsafe extern "system" fn(lpFlsData: ::PVOID)>;
pub type PSECURE_MEMORY_CACHE_CALLBACK = Option<unsafe extern "system" fn(
    Addr: ::PVOID, Range: ::SIZE_T,
) -> ::BOOLEAN>;
pub const WT_EXECUTEINLONGTHREAD: ::ULONG = 0x00000010;
pub const WT_EXECUTEDELETEWAIT: ::ULONG = 0x00000008;
//18570
pub const KEY_QUERY_VALUE: ::REGSAM = 0x0001;
pub const KEY_SET_VALUE: ::REGSAM = 0x0002;
pub const KEY_CREATE_SUB_KEY: ::REGSAM = 0x0004;
pub const KEY_ENUMERATE_SUB_KEYS: ::REGSAM = 0x0008;
pub const KEY_NOTIFY: ::REGSAM = 0x0010;
pub const KEY_CREATE_LINK: ::REGSAM = 0x0020;
pub const KEY_WOW64_32KEY: ::REGSAM = 0x0200;
pub const KEY_WOW64_64KEY: ::REGSAM = 0x0100;
pub const KEY_WOW64_RES: ::REGSAM = 0x0300;
pub const KEY_READ: ::REGSAM = (
        STANDARD_RIGHTS_READ |
        KEY_QUERY_VALUE |
        KEY_ENUMERATE_SUB_KEYS |
        KEY_NOTIFY
    ) & (!SYNCHRONIZE);
pub const KEY_WRITE: ::REGSAM = (STANDARD_RIGHTS_WRITE | KEY_SET_VALUE | KEY_CREATE_SUB_KEY) & (!SYNCHRONIZE);
pub const KEY_EXECUTE: ::REGSAM = KEY_READ & (!SYNCHRONIZE);
pub const KEY_ALL_ACCESS: ::REGSAM = (
        STANDARD_RIGHTS_ALL |
        KEY_QUERY_VALUE |
        KEY_SET_VALUE |
        KEY_CREATE_SUB_KEY |
        KEY_ENUMERATE_SUB_KEYS |
        KEY_NOTIFY |
        KEY_CREATE_LINK
    ) & (!SYNCHRONIZE);
pub const REG_CREATED_NEW_KEY: ::DWORD = 0x00000001;
pub const REG_OPENED_EXISTING_KEY: ::DWORD = 0x00000002;
pub const REG_NOTIFY_CHANGE_NAME: ::DWORD = 0x00000001;
pub const REG_NOTIFY_CHANGE_ATTRIBUTES: ::DWORD = 0x00000002;
pub const REG_NOTIFY_CHANGE_LAST_SET: ::DWORD = 0x00000004;
pub const REG_NOTIFY_CHANGE_SECURITY: ::DWORD = 0x00000008;
pub const REG_LEGAL_CHANGE_FILTER: ::DWORD = REG_NOTIFY_CHANGE_NAME |
    REG_NOTIFY_CHANGE_ATTRIBUTES |
    REG_NOTIFY_CHANGE_LAST_SET |
    REG_NOTIFY_CHANGE_SECURITY;
pub const REG_NOTIFY_THREAD_AGNOSTIC: ::DWORD = 0x10000000; //supported only on Windows 8 and later
pub const REG_OPTION_RESERVED: ::DWORD = 0x00000000;
pub const REG_OPTION_NON_VOLATILE: ::DWORD = 0x00000000;
pub const REG_OPTION_VOLATILE: ::DWORD = 0x00000001;
pub const REG_OPTION_CREATE_LINK: ::DWORD = 0x00000002;
pub const REG_OPTION_BACKUP_RESTORE: ::DWORD = 0x00000004;
pub const REG_OPTION_OPEN_LINK: ::DWORD = 0x00000008;
pub const REG_NONE: ::DWORD = 0;
pub const REG_SZ: ::DWORD = 1;
pub const REG_EXPAND_SZ: ::DWORD = 2;
pub const REG_BINARY: ::DWORD = 3;
pub const REG_DWORD: ::DWORD = 4;
pub const REG_DWORD_LITTLE_ENDIAN: ::DWORD = 4;
pub const REG_DWORD_BIG_ENDIAN: ::DWORD = 5;
pub const REG_LINK: ::DWORD = 6;
pub const REG_MULTI_SZ: ::DWORD = 7;
pub const REG_RESOURCE_LIST: ::DWORD = 8;
pub const REG_FULL_RESOURCE_DESCRIPTOR: ::DWORD = 9;
pub const REG_RESOURCE_REQUIREMENTS_LIST: ::DWORD = 10;
pub const REG_QWORD: ::DWORD = 11;
pub const REG_QWORD_LITTLE_ENDIAN: ::DWORD = 11;
//18720
pub const SERVICE_KERNEL_DRIVER: ::DWORD = 0x00000001;
pub const SERVICE_FILE_SYSTEM_DRIVER: ::DWORD = 0x00000002;
pub const SERVICE_ADAPTER: ::DWORD = 0x00000004;
pub const SERVICE_RECOGNIZER_DRIVER: ::DWORD = 0x00000008;
pub const SERVICE_DRIVER: ::DWORD = SERVICE_KERNEL_DRIVER | SERVICE_FILE_SYSTEM_DRIVER
    | SERVICE_RECOGNIZER_DRIVER;
pub const SERVICE_WIN32_OWN_PROCESS: ::DWORD = 0x00000010;
pub const SERVICE_WIN32_SHARE_PROCESS: ::DWORD = 0x00000020;
pub const SERVICE_WIN32: ::DWORD = SERVICE_WIN32_OWN_PROCESS | SERVICE_WIN32_SHARE_PROCESS;
pub const SERVICE_INTERACTIVE_PROCESS: ::DWORD = 0x00000100;
pub const SERVICE_TYPE_ALL: ::DWORD = SERVICE_WIN32 | SERVICE_ADAPTER | SERVICE_DRIVER
    | SERVICE_INTERACTIVE_PROCESS;
STRUCT!{struct TP_CALLBACK_INSTANCE {
    dummy: *mut ::c_void,
}}
pub type PTP_CALLBACK_INSTANCE = *mut TP_CALLBACK_INSTANCE;
STRUCT!{struct TP_IO {
    dummy: *mut ::c_void,
}}
pub type PTP_IO = *mut TP_IO;
STRUCT!{struct TP_POOL {
    dummy: *mut ::c_void,
}}
pub type PTP_POOL = *mut TP_POOL;
STRUCT!{struct TP_CLEANUP_GROUP {
    dummy: *mut ::c_void,
}}
pub type PTP_CLEANUP_GROUP = *mut TP_CLEANUP_GROUP;
STRUCT!{struct TP_TIMER {
    dummy: *mut ::c_void,
}}
pub type PTP_TIMER = *mut TP_TIMER;
STRUCT!{struct TP_WAIT {
    dummy: *mut ::c_void,
}}
pub type PTP_WAIT = *mut TP_WAIT;
STRUCT!{struct TP_WORK {
    dummy: *mut ::c_void,
}}
pub type PTP_WORK = *mut TP_WORK;
STRUCT!{struct ACTIVATION_CONTEXT {
    dummy: *mut ::c_void,
}}
ENUM!{enum TP_CALLBACK_PRIORITY {
    TP_CALLBACK_PRIORITY_HIGH,
    TP_CALLBACK_PRIORITY_NORMAL,
    TP_CALLBACK_PRIORITY_LOW,
    TP_CALLBACK_PRIORITY_INVALID,
    TP_CALLBACK_PRIORITY_COUNT = 4,
}}
pub type PTP_CLEANUP_GROUP_CANCEL_CALLBACK = Option<unsafe extern "system" fn(
    ObjectContext: ::PVOID, CleanupContext: ::PVOID,
)>;
pub type PTP_SIMPLE_CALLBACK = Option<unsafe extern "system" fn(
    Instance: PTP_CALLBACK_INSTANCE, Context: ::PVOID,
)>;
pub type PTP_WORK_CALLBACK = Option<unsafe extern "system" fn(
    Instance: PTP_CALLBACK_INSTANCE, Context: ::PVOID, Work: PTP_WORK,
)>;
pub type PTP_TIMER_CALLBACK = Option<unsafe extern "system" fn(
    Instance: PTP_CALLBACK_INSTANCE, Context: ::PVOID, Timer: PTP_TIMER,
)>;
pub type TP_WAIT_RESULT = ::DWORD;
pub type PTP_WAIT_CALLBACK = Option<unsafe extern "system" fn(
    Instance: PTP_CALLBACK_INSTANCE, Context: ::PVOID, Wait: PTP_WAIT, WaitResult: TP_WAIT_RESULT,
)>;
pub type TP_VERSION = ::DWORD;
pub type PTP_VERSION = *mut ::DWORD;
STRUCT!{struct TP_POOL_STACK_INFORMATION {
    StackReserve: ::SIZE_T,
    StackCommit: ::SIZE_T,
}}
pub type PTP_POOL_STACK_INFORMATION = *mut TP_POOL_STACK_INFORMATION;
STRUCT!{struct TP_CALLBACK_ENVIRON_V3_s {
    BitFields: ::DWORD,
}}
BITFIELD!(TP_CALLBACK_ENVIRON_V3_s BitFields: ::DWORD [
    LongFunction set_LongFunction[0..1],
    Persistent set_Persistent[1..2],
    Private set_Private[2..32],
]);
STRUCT!{nodebug struct TP_CALLBACK_ENVIRON_V3 {
    Version: TP_VERSION,
    Pool: PTP_POOL,
    CleanupGroup: PTP_CLEANUP_GROUP,
    CleanupGroupCancelCallback: PTP_CLEANUP_GROUP_CANCEL_CALLBACK,
    RaceDll: ::PVOID,
    ActivationContext: *mut ACTIVATION_CONTEXT,
    FinalizationCallback: PTP_SIMPLE_CALLBACK,
    u: ::DWORD,
    CallbackPriority: TP_CALLBACK_PRIORITY,
    Size: ::DWORD,
}}
UNION!(TP_CALLBACK_ENVIRON_V3, u, Flags, Flags_mut, ::DWORD);
UNION!(TP_CALLBACK_ENVIRON_V3, u, s, s_mut, TP_CALLBACK_ENVIRON_V3_s);
pub type TP_CALLBACK_ENVIRON = TP_CALLBACK_ENVIRON_V3;
pub type PTP_CALLBACK_ENVIRON = *mut TP_CALLBACK_ENVIRON_V3;
STRUCT!{struct JOB_SET_ARRAY {
    JobHandle: ::HANDLE,
    MemberLevel: ::DWORD,
    Flags: ::DWORD,
}}
pub type PJOB_SET_ARRAY = *mut JOB_SET_ARRAY;
STRUCT!{struct RTL_BARRIER {
    Reserved1: ::DWORD,
    Reserved2: ::DWORD,
    Reserved3: [::ULONG_PTR; 2],
    Reserved4: ::DWORD,
    Reserved5: ::DWORD,
}}
pub type PRTL_BARRIER = *mut RTL_BARRIER;
STRUCT!{struct RTL_RUN_ONCE {
    Ptr: ::PVOID,
}}
pub type PRTL_RUN_ONCE = *mut RTL_RUN_ONCE;
ENUM!{enum RTL_UMS_THREAD_INFO_CLASS {
    UmsThreadInvalidInfoClass = 0,
    UmsThreadUserContext,
    UmsThreadPriority,              // Reserved
    UmsThreadAffinity,              // Reserved
    UmsThreadTeb,
    UmsThreadIsSuspended,
    UmsThreadIsTerminated,
    UmsThreadMaxInfoClass,
}}
ENUM!{enum RTL_UMS_SCHEDULER_REASON {
    UmsSchedulerStartup = 0,
    UmsSchedulerThreadBlocked,
    UmsSchedulerThreadYield,
}}
pub type PRTL_UMS_SCHEDULER_ENTRY_POINT = Option<unsafe extern "system" fn(
    Reason: RTL_UMS_SCHEDULER_REASON, ActivationPayload: ::ULONG_PTR, SchedulerParam: ::PVOID,
)>;
ENUM!{enum FIRMWARE_TYPE {
    FirmwareTypeUnknown,
    FirmwareTypeBios,
    FirmwareTypeUefi,
    FirmwareTypeMax,
}}
pub type PFIRMWARE_TYPE = *mut FIRMWARE_TYPE;
ENUM!{enum LOGICAL_PROCESSOR_RELATIONSHIP {
    RelationProcessorCore,
    RelationNumaNode,
    RelationCache,
    RelationProcessorPackage,
    RelationGroup,
    RelationAll = 0xffff,
}}
ENUM!{enum PROCESSOR_CACHE_TYPE {
    CacheUnified,
    CacheInstruction,
    CacheData,
    CacheTrace,
}}
STRUCT!{struct CACHE_DESCRIPTOR {
    Level: ::BYTE,
    Associativity: ::BYTE,
    LineSize: ::WORD,
    Size: ::DWORD,
    Type: PROCESSOR_CACHE_TYPE,
}}
pub type PCACHE_DESCRIPTOR = *mut CACHE_DESCRIPTOR;
STRUCT!{struct SYSTEM_LOGICAL_PROCESSOR_INFORMATION_ProcessorCore {
    Flags: ::BYTE,
}}
STRUCT!{struct SYSTEM_LOGICAL_PROCESSOR_INFORMATION_NumaNode {
    NodeNumber: ::DWORD,
}}
STRUCT!{struct SYSTEM_LOGICAL_PROCESSOR_INFORMATION {
    ProcessorMask: ::ULONG_PTR,
    Relationship: LOGICAL_PROCESSOR_RELATIONSHIP,
    Reserved: [::ULONGLONG; 2],
}}
UNION!(
    SYSTEM_LOGICAL_PROCESSOR_INFORMATION, Reserved, ProcessorCore, ProcessorCore_mut,
    SYSTEM_LOGICAL_PROCESSOR_INFORMATION_ProcessorCore
);
UNION!(
    SYSTEM_LOGICAL_PROCESSOR_INFORMATION, Reserved, NumaNode, NumaNode_mut,
    SYSTEM_LOGICAL_PROCESSOR_INFORMATION_NumaNode
);
UNION!(SYSTEM_LOGICAL_PROCESSOR_INFORMATION, Reserved, Cache, Cache_mut, CACHE_DESCRIPTOR);
pub type PSYSTEM_LOGICAL_PROCESSOR_INFORMATION = *mut SYSTEM_LOGICAL_PROCESSOR_INFORMATION;
STRUCT!{struct SYSTEM_PROCESSOR_CYCLE_TIME_INFORMATION {
    CycleTime: ::DWORD64,
}}
pub type PSYSTEM_PROCESSOR_CYCLE_TIME_INFORMATION = *mut SYSTEM_PROCESSOR_CYCLE_TIME_INFORMATION;
ENUM!{enum HARDWARE_COUNTER_TYPE {
    PMCCounter,
    MaxHardwareCounterType,
}}
pub type PHARDWARE_COUNTER_TYPE = *mut HARDWARE_COUNTER_TYPE;
ENUM!{enum PROCESS_MITIGATION_POLICY {
    ProcessDEPPolicy,
    ProcessASLRPolicy,
    ProcessDynamicCodePolicy,
    ProcessStrictHandleCheckPolicy,
    ProcessSystemCallDisablePolicy,
    ProcessMitigationOptionsMask,
    ProcessExtensionPointDisablePolicy,
    ProcessReserved1Policy,
    ProcessSignaturePolicy,
    MaxProcessMitigationPolicy,
}}
STRUCT!{nodebug struct OSVERSIONINFOA {
    dwOSVersionInfoSize: ::DWORD,
    dwMajorVersion: ::DWORD,
    dwMinorVersion: ::DWORD,
    dwBuildNumber: ::DWORD,
    dwPlatformId: ::DWORD,
    szCSDVersion: [::CHAR; 128],
}}
pub type POSVERSIONINFOA = *mut OSVERSIONINFOA;
pub type LPOSVERSIONINFOA = *mut OSVERSIONINFOA;
STRUCT!{nodebug struct OSVERSIONINFOW {
    dwOSVersionInfoSize: ::DWORD,
    dwMajorVersion: ::DWORD,
    dwMinorVersion: ::DWORD,
    dwBuildNumber: ::DWORD,
    dwPlatformId: ::DWORD,
    szCSDVersion: [::WCHAR; 128],
}}
pub type POSVERSIONINFOW = *mut OSVERSIONINFOW;
pub type LPOSVERSIONINFOW = *mut OSVERSIONINFOW;
STRUCT!{nodebug struct OSVERSIONINFOEXA {
    dwOSVersionInfoSize: ::DWORD,
    dwMajorVersion: ::DWORD,
    dwMinorVersion: ::DWORD,
    dwBuildNumber: ::DWORD,
    dwPlatformId: ::DWORD,
    szCSDVersion: [::CHAR; 128],
    wServicePackMajor: ::WORD,
    wServicePackMinor: ::WORD,
    wSuiteMask: ::WORD,
    wProductType: ::BYTE,
    wReserved: ::BYTE,
}}
pub type POSVERSIONINFOEXA = *mut OSVERSIONINFOEXA;
pub type LPOSVERSIONINFOEXA = *mut OSVERSIONINFOEXA;
STRUCT!{nodebug struct OSVERSIONINFOEXW {
    dwOSVersionInfoSize: ::DWORD,
    dwMajorVersion: ::DWORD,
    dwMinorVersion: ::DWORD,
    dwBuildNumber: ::DWORD,
    dwPlatformId: ::DWORD,
    szCSDVersion: [::WCHAR; 128],
    wServicePackMajor: ::WORD,
    wServicePackMinor: ::WORD,
    wSuiteMask: ::WORD,
    wProductType: ::BYTE,
    wReserved: ::BYTE,
}}
pub type POSVERSIONINFOEXW = *mut OSVERSIONINFOEXW;
pub type LPOSVERSIONINFOEXW = *mut OSVERSIONINFOEXW;
STRUCT!{struct SLIST_ENTRY {
    Next: *mut SLIST_ENTRY,
}}
pub type PSLIST_ENTRY = *mut SLIST_ENTRY;
STRUCT!{struct SLIST_HEADER_HeaderX64 {
    BitFields1: ::ULONGLONG,
    BitFields2: ::ULONGLONG,
}}
BITFIELD!(SLIST_HEADER_HeaderX64 BitFields1: ::ULONGLONG [
    Depth set_Depth[0..16],
    Sequence set_Sequence[16..64],
]);
BITFIELD!(SLIST_HEADER_HeaderX64 BitFields2: ::ULONGLONG [
    Reserved set_Reserved[0..4],
    NextEntry set_NextEntry[4..64],
]);
STRUCT!{struct SLIST_HEADER {
    Alignment: ::ULONGLONG,
    Region: ::ULONGLONG,
}}
UNION!(SLIST_HEADER, Alignment, HeaderX64, HeaderX64_mut, SLIST_HEADER_HeaderX64);
pub type PSLIST_HEADER = *mut SLIST_HEADER;
ENUM!{enum SYSTEM_POWER_STATE {
    PowerSystemUnspecified = 0,
    PowerSystemWorking = 1,
    PowerSystemSleeping1 = 2,
    PowerSystemSleeping2 = 3,
    PowerSystemSleeping3 = 4,
    PowerSystemHibernate = 5,
    PowerSystemShutdown = 6,
    PowerSystemMaximum = 7,
}}
pub type PSYSTEM_POWER_STATE = *mut SYSTEM_POWER_STATE;
ENUM!{enum POWER_ACTION {
    PowerActionNone = 0,
    PowerActionReserved,
    PowerActionSleep,
    PowerActionHibernate,
    PowerActionShutdown,
    PowerActionShutdownReset,
    PowerActionShutdownOff,
    PowerActionWarmEject,
}}
pub type PPOWER_ACTION = *mut POWER_ACTION;
ENUM!{enum DEVICE_POWER_STATE {
    PowerDeviceUnspecified = 0,
    PowerDeviceD0,
    PowerDeviceD1,
    PowerDeviceD2,
    PowerDeviceD3,
    PowerDeviceMaximum,
}}
pub type PDEVICE_POWER_STATE = *mut DEVICE_POWER_STATE;
ENUM!{enum MONITOR_DISPLAY_STATE {
    PowerMonitorOff = 0,
    PowerMonitorOn,
    PowerMonitorDim,
}}
pub type PMONITOR_DISPLAY_STATE = *mut MONITOR_DISPLAY_STATE;
ENUM!{enum USER_ACTIVITY_PRESENCE {
    PowerUserPresent = 0,
    PowerUserNotPresent,
    PowerUserInactive,
    PowerUserMaximum,
    //PowerUserInvalid = 3,
}}
pub type PUSER_ACTIVITY_PRESENCE = *mut USER_ACTIVITY_PRESENCE;
pub type EXECUTION_STATE = ::DWORD;
pub type PEXECUTION_STATE = *mut ::DWORD;
ENUM!{enum LATENCY_TIME {
    LT_DONT_CARE,
    LT_LOWEST_LATENCY,
}}
ENUM!{enum POWER_REQUEST_TYPE {
    PowerRequestDisplayRequired,
    PowerRequestSystemRequired,
    PowerRequestAwayModeRequired,
    PowerRequestExecutionRequired,
}}
pub type PPOWER_REQUEST_TYPE = *mut POWER_REQUEST_TYPE;
pub const MAX_HW_COUNTERS: usize = 16;
STRUCT!{struct HARDWARE_COUNTER_DATA {
    Type: HARDWARE_COUNTER_TYPE,
    Reserved: ::DWORD,
    Value: ::DWORD64,
}}
pub type PHARDWARE_COUNTER_DATA = *mut HARDWARE_COUNTER_DATA;
STRUCT!{struct PERFORMANCE_DATA {
    Size: ::WORD,
    Version: ::BYTE,
    HwCountersCount: ::BYTE,
    ContextSwitchCount: ::DWORD,
    WaitReasonBitMap: ::DWORD64,
    CycleTime: ::DWORD64,
    RetryCount: ::DWORD,
    Reserved: ::DWORD,
    HwCounters: [HARDWARE_COUNTER_DATA; MAX_HW_COUNTERS],
}}
pub type PPERFORMANCE_DATA = *mut PERFORMANCE_DATA;
STRUCT!{struct MEMORY_BASIC_INFORMATION {
    BaseAddress: ::PVOID,
    AllocationBase: ::PVOID,
    AllocationProtect: ::DWORD,
    RegionSize: ::SIZE_T,
    State: ::DWORD,
    Protect: ::DWORD,
    Type: ::DWORD,
}}
pub type PMEMORY_BASIC_INFORMATION = *mut MEMORY_BASIC_INFORMATION;
STRUCT!{struct MEMORY_BASIC_INFORMATION32 {
    BaseAddress: ::DWORD,
    AllocationBase: ::DWORD,
    AllocationProtect: ::DWORD,
    RegionSize: ::DWORD,
    State: ::DWORD,
    Protect: ::DWORD,
    Type: ::DWORD,
}}
pub type PMEMORY_BASIC_INFORMATION32 = *mut MEMORY_BASIC_INFORMATION32;
STRUCT!{struct MEMORY_BASIC_INFORMATION64 { // FIXME: align 16
    BaseAddress: ::ULONGLONG,
    AllocationBase: ::ULONGLONG,
    AllocationProtect: ::DWORD,
    __alignment1: ::DWORD,
    RegionSize: ::ULONGLONG,
    State: ::DWORD,
    Protect: ::DWORD,
    Type: ::DWORD,
    __alignment2: ::DWORD,
}}
pub type PMEMORY_BASIC_INFORMATION64 = *mut MEMORY_BASIC_INFORMATION64;
pub const WOW64_SIZE_OF_80387_REGISTERS: usize = 80;
pub const WOW64_MAXIMUM_SUPPORTED_EXTENSION: usize = 512;
STRUCT!{nodebug struct WOW64_FLOATING_SAVE_AREA {
    ControlWord: ::DWORD,
    StatusWord: ::DWORD,
    TagWord: ::DWORD,
    ErrorOffset: ::DWORD,
    ErrorSelector: ::DWORD,
    DataOffset: ::DWORD,
    DataSelector: ::DWORD,
    RegisterArea: [::BYTE; WOW64_SIZE_OF_80387_REGISTERS],
    Cr0NpxState: ::DWORD,
}}
pub type PWOW64_FLOATING_SAVE_AREA = *mut WOW64_FLOATING_SAVE_AREA;
STRUCT!{nodebug struct WOW64_CONTEXT {
    ContextFlags: ::DWORD,
    Dr0: ::DWORD,
    Dr1: ::DWORD,
    Dr2: ::DWORD,
    Dr3: ::DWORD,
    Dr4: ::DWORD,
    Dr5: ::DWORD,
    Dr6: ::DWORD,
    Dr7: ::DWORD,
    FloatSave: WOW64_FLOATING_SAVE_AREA,
    SegGs: ::DWORD,
    SegFs: ::DWORD,
    SegEs: ::DWORD,
    SegDs: ::DWORD,
    Edi: ::DWORD,
    Esi: ::DWORD,
    Ebx: ::DWORD,
    Edx: ::DWORD,
    Ecx: ::DWORD,
    Eax: ::DWORD,
    Ebp: ::DWORD,
    Eip: ::DWORD,
    SegCs: ::DWORD,
    EFlags: ::DWORD,
    Esp: ::DWORD,
    SegSs: ::DWORD,
    ExtendedRegisters: [::BYTE; WOW64_MAXIMUM_SUPPORTED_EXTENSION],
}}
pub type PWOW64_CONTEXT = *mut WOW64_CONTEXT;
STRUCT!{struct WOW64_LDT_ENTRY_Bytes {
    BaseMid: ::BYTE,
    Flags1: ::BYTE,
    Flags2: ::BYTE,
    BaseHi: ::BYTE,
}}
STRUCT!{struct WOW64_LDT_ENTRY_Bits {
    BitFields: ::DWORD,
}}
BITFIELD!(WOW64_LDT_ENTRY_Bits BitFields: ::DWORD [
    BaseMid set_BaseMid[0..8],
    Type set_Type[8..13],
    Dpl set_Dpl[13..15],
    Pres set_Pres[15..16],
    LimitHi set_LimitHi[16..20],
    Sys set_Sys[20..21],
    Reserved_0 set_Reserved_0[21..22],
    Default_Big set_Default_Big[22..23],
    Granularity set_Granularity[23..24],
    BaseHi set_BaseHi[24..32],
]);
STRUCT!{struct WOW64_LDT_ENTRY {
    LimitLow: ::WORD,
    BaseLow: ::WORD,
    HighWord: ::DWORD,
}}
UNION!(WOW64_LDT_ENTRY, HighWord, Bytes, Bytes_mut, WOW64_LDT_ENTRY_Bytes);
UNION!(WOW64_LDT_ENTRY, HighWord, Bits, Bits_mut, WOW64_LDT_ENTRY_Bits);
pub type PWOW64_LDT_ENTRY = *mut WOW64_LDT_ENTRY;

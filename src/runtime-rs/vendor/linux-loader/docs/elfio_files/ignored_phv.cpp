/*
Copyright (C) 2001-present by Serge Lamikhov-Center

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in
all copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
THE SOFTWARE.
*/

#include <elfio/elfio.hpp>

using namespace ELFIO;

int main( void )
{
    elfio writer;

    // You can't proceed without this function call!
    writer.create( ELFCLASS64, ELFDATA2LSB );

    writer.set_os_abi( ELFOSABI_LINUX );
    writer.set_type( ET_EXEC );
    writer.set_machine( EM_X86_64 );

    // Create a loadable segment
    segment* load_seg = writer.segments.add();
    load_seg->set_type( PT_LOAD );
    load_seg->set_virtual_address( 0x400000 );
    load_seg->set_physical_address( 0x400000 );
    load_seg->set_flags( PF_R );
    load_seg->set_align( 0x200000 );

    // Create a note segment
    segment* note_seg = writer.segments.add();
    note_seg->set_type( PT_NOTE );
    note_seg->set_virtual_address( 0x4000b0 );
    note_seg->set_physical_address( 0x4000b0 );
    note_seg->set_flags( PF_R );
    note_seg->set_align( 0x4 );

    // Create a .note.dummy section, and add it to the note segment.
    section* dummy_note_sec = writer.sections.add( ".note.dummy" );
    dummy_note_sec->set_type( SHT_NOTE );
    dummy_note_sec->set_addr_align( 0x4 );
    dummy_note_sec->set_flags( SHF_ALLOC );
    note_section_accessor dummy_note_writer( writer, dummy_note_sec );

    unsigned char dummy_desc[8] = { 0xfe, 0xca, 0xfe, 0xca, 0x00, 0x00 };
    dummy_note_writer.add_note( 0x01, "dummy", dummy_desc, sizeof( dummy_desc ) );

    note_seg->add_section_index( dummy_note_sec->get_index(),
                                 dummy_note_sec->get_addr_align() );

    // Create a .note.Xen section, and add it to the note segment.
    section* xen_note_sec = writer.sections.add( ".note.Xen" );
    xen_note_sec->set_type( SHT_NOTE );
    xen_note_sec->set_addr_align( 0x4 );
    xen_note_sec->set_flags( SHF_ALLOC );
    note_section_accessor xen_note_writer( writer, xen_note_sec );

    unsigned char xen_descr[8] = { 0x1f, 0xfe, 0xe1, 0x01 };
    xen_note_writer.add_note( 0x12, "Xen", xen_descr, sizeof( xen_descr ) );

    note_seg->add_section_index( xen_note_sec->get_index(),
                                 xen_note_sec->get_addr_align() );

    // Create a .note.gnu.build-id section, and add it to the note segment.
    section* gnu_note_sec = writer.sections.add( ".note.gnu.build-id" );
    gnu_note_sec->set_type( SHT_NOTE );
    gnu_note_sec->set_addr_align( 0x4 );
    gnu_note_sec->set_flags( SHF_ALLOC );
    note_section_accessor gnu_note_writer( writer, gnu_note_sec );

    unsigned char gnu_descr[20] = { 0x28, 0xcc, 0x3d, 0x3d, 0x89, 0xe5, 0xbf,
                                    0xc6, 0x07, 0xa8, 0xce, 0xe3, 0x29, 0xcc,
                                    0x70, 0xd0, 0xbf, 0x34, 0x69, 0x2b };
    gnu_note_writer.add_note( 0x03, "GNU", gnu_descr, sizeof( gnu_descr ) );

    note_seg->add_section_index( gnu_note_sec->get_index(),
                                 gnu_note_sec->get_addr_align() );
    // Setup entry point. Usually, a linker sets this address on base of
    // ‘_start’ label.
    writer.set_entry( 0x400108 );

    // Create ELF file
    writer.save( "test_elfnote.bin" );

    return 0;
}

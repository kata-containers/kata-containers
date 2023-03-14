#!/bin/sh

set -e

SHA1SUM=$(which sha1sum)
SHA1CDSUM=$(readlink -f target/debug/sha1cdsum)

cd $(mktemp -d)

mkdir data
cd data
for i in 'a' ' b' '*c' 'zz' ' ' 'foo\n' '\nfoo' '\\' '\\\\'
do
    N="$(/bin/echo -e "$i")"
    echo "$N" > "$N"
done

# Produce different checksum files.
for TOOL in $SHA1SUM $SHA1CDSUM
do
    T=$(basename $TOOL)
    $TOOL * > ../SUM.$T
    $TOOL * --binary > ../SUM.$T.binary
    $TOOL * --text > ../SUM.$T.text
    $TOOL * --tag > ../SUM.$T.tag
    $TOOL * --tag --binary > ../SUM.$T.tag.binary
    $TOOL * --zero > ../SUM.$T.zero
done

# And make sure both tools output the same.
cd ..
for WHAT in "" .binary .text .tag .tag.binary .zero
do
    diff -u SUM.sha1sum$WHAT SUM.sha1cdsum$WHAT
done

# Now check the various checksum files with both tools and compare the
# output.
cd data
for WHAT in "" .binary .text .tag .tag.binary
do
    $SHA1SUM --check ../SUM.sha1sum$WHAT >../CHECK.sha1sum.out$WHAT
    $SHA1CDSUM --check ../SUM.sha1sum$WHAT >../CHECK.sha1cdsum.out$WHAT
    diff -u ../CHECK.sha1sum.out$WHAT ../CHECK.sha1cdsum.out$WHAT
done

# Now check how missing files are handled.
cd ..
for F in SUM.*
do
    sed -e s/zz/yy/ $F > $F.missing
done
cd data
for WHAT in "" .binary .text .tag .tag.binary
do
    set +e
    $SHA1SUM --check ../SUM.sha1sum$WHAT.missing >../CHECK.sha1sum.out$WHAT
    A=$?
    $SHA1CDSUM --check ../SUM.sha1sum$WHAT.missing >../CHECK.sha1cdsum.out$WHAT
    B=$?
    set -e
    test $A = $B
    diff -u ../CHECK.sha1sum.out$WHAT ../CHECK.sha1cdsum.out$WHAT

    $SHA1SUM --check --ignore-missing ../SUM.sha1sum$WHAT.missing >../CHECK.sha1sum.out$WHAT
    $SHA1CDSUM --check --ignore-missing ../SUM.sha1sum$WHAT.missing >../CHECK.sha1cdsum.out$WHAT
    diff -u ../CHECK.sha1sum.out$WHAT ../CHECK.sha1cdsum.out$WHAT
done

# Only one file, and it is missing.
cd ..
for F in SUM.*.missing
do
    grep yy $F > $F.single-missing
done
cd data
for WHAT in "" .binary .text .tag .tag.binary
do
    set +e
    $SHA1SUM --check --ignore-missing ../SUM.sha1sum$WHAT.single-missing >../CHECK.sha1sum.out$WHAT
    A=$?
    $SHA1CDSUM --check --ignore-missing ../SUM.sha1sum$WHAT.single-missing >../CHECK.sha1cdsum.out$WHAT
    B=$?
    set -e
    test $A = $B
    diff -u ../CHECK.sha1sum.out$WHAT ../CHECK.sha1cdsum.out$WHAT
done

# Comments.
cd ..
for F in SUM.*
do
    (echo "# Comment" ; cat $F ; echo -n "#Comment") > $F.comment
done
cd data
for WHAT in "" .binary .text .tag .tag.binary
do
    $SHA1SUM --check ../SUM.sha1sum$WHAT.comment >../CHECK.sha1sum.out$WHAT
    $SHA1CDSUM --check ../SUM.sha1sum$WHAT.comment >../CHECK.sha1cdsum.out$WHAT
    diff -u ../CHECK.sha1sum.out$WHAT ../CHECK.sha1cdsum.out$WHAT
done

# Garbage.
cd ..
for F in SUM.*
do
    (echo "Garbage" ; cat $F ; echo -n "More garbage") > $F.garbage
done
cd data
for WHAT in "" .binary .text .tag .tag.binary
do
    set +e
    $SHA1SUM --check ../SUM.sha1sum$WHAT.garbage >../CHECK.sha1sum.out$WHAT
    A=$?
    $SHA1CDSUM --check ../SUM.sha1sum$WHAT.garbage >../CHECK.sha1cdsum.out$WHAT
    B=$?
    set -e
    test $A = $B
    diff -u ../CHECK.sha1sum.out$WHAT ../CHECK.sha1cdsum.out$WHAT
done

# Now change one of the files.
echo >> zz
for WHAT in "" .binary .text .tag .tag.binary
do
    set +e
    $SHA1SUM --check ../SUM.sha1sum$WHAT >../CHECK.sha1sum.out$WHAT
    A=$?
    $SHA1CDSUM --check ../SUM.sha1sum$WHAT >../CHECK.sha1cdsum.out$WHAT
    B=$?
    set -e
    test $A = $B
    diff -u ../CHECK.sha1sum.out$WHAT ../CHECK.sha1cdsum.out$WHAT
done

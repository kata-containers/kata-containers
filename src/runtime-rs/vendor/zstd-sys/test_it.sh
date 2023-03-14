#!/bin/sh
for EXP in "experimental" ""; do
    for STD in "std" ""; do
        cargo test --features "$EXP $STD"
    done
done


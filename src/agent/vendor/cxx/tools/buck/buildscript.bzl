def buildscript_args(
        name,
        package_name,
        buildscript_rule,
        cfgs,
        features,
        outfile,
        version):
    native.genrule(
        name = name,
        out = outfile,
        cmd = "env RUSTC=rustc TARGET= $(exe %s) | sed -n s/^cargo:rustc-cfg=/--cfg=/p > ${OUT}" % buildscript_rule,
    )

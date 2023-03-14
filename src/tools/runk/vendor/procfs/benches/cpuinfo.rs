use criterion::{black_box, criterion_group, criterion_main, Criterion};
use procfs::CpuInfo;

fn bench_cpuinfo(c: &mut Criterion) {
    c.bench_function("CpuInfo::new", |b| b.iter(|| black_box(CpuInfo::new().unwrap())));

    let cpuinfo = black_box(CpuInfo::new().unwrap());
    c.bench_function("CpuInfo::get_info", |b| b.iter(|| black_box(cpuinfo.get_info(0))));
    c.bench_function("CpuInfo::model_name", |b| b.iter(|| cpuinfo.model_name(0)));
    c.bench_function("CpuInfo::vendor_id", |b| b.iter(|| cpuinfo.vendor_id(0)));
    c.bench_function("CpuInfo::physical_id", |b| b.iter(|| cpuinfo.physical_id(0)));
    c.bench_function("CpuInfo::flags", |b| b.iter(|| cpuinfo.flags(0)));
}

criterion_group!(benches, bench_cpuinfo);
criterion_main!(benches);

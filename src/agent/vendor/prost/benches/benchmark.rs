use std::fs::File;
use std::io::Read;
use std::result;

use criterion::{Benchmark, Criterion, Throughput};
use failure::bail;
use prost::Message;
use protobuf::benchmarks::{proto2, proto3, BenchmarkDataset};

type Result = result::Result<(), failure::Error>;

fn benchmark_dataset<M>(criterion: &mut Criterion, dataset: BenchmarkDataset) -> Result
where
    M: prost::Message + Default + 'static,
{
    let payload_len = dataset.payload.iter().map(Vec::len).sum::<usize>();

    let messages = dataset
        .payload
        .iter()
        .map(|buf| M::decode(buf))
        .collect::<result::Result<Vec<_>, _>>()?;
    let encoded_len = messages
        .iter()
        .map(|message| message.encoded_len())
        .sum::<usize>();

    let mut buf = Vec::with_capacity(encoded_len);
    let encode = Benchmark::new("encode", move |b| {
        b.iter(|| {
            buf.clear();
            for message in &messages {
                message.encode(&mut buf).unwrap();
            }
            criterion::black_box(&buf);
        })
    })
    .throughput(Throughput::Bytes(encoded_len as u32));

    let payload = dataset.payload.clone();
    let decode = Benchmark::new("decode", move |b| {
        b.iter(|| {
            for buf in &payload {
                criterion::black_box(M::decode(buf).unwrap());
            }
        })
    })
    .throughput(Throughput::Bytes(payload_len as u32));

    let payload = dataset.payload.clone();
    let merge = Benchmark::new("merge", move |b| {
        let mut message = M::default();
        b.iter(|| {
            for buf in &payload {
                message.clear();
                message.merge(buf).unwrap();
                criterion::black_box(&message);
            }
        })
    })
    .throughput(Throughput::Bytes(payload_len as u32));

    criterion
        .bench(&dataset.name, encode)
        .bench(&dataset.name, decode)
        .bench(&dataset.name, merge);

    Ok(())
}

fn main() -> Result {
    let mut criterion = Criterion::default().configure_from_args();

    for dataset in protobuf::benchmarks::datasets() {
        let dataset = {
            let mut f = File::open(dataset)?;
            let mut buf = Vec::new();
            f.read_to_end(&mut buf)?;
            protobuf::benchmarks::BenchmarkDataset::decode(buf)?
        };

        match dataset.message_name.as_str() {
            "benchmarks.proto2.GoogleMessage1" => {
                benchmark_dataset::<proto2::GoogleMessage1>(&mut criterion, dataset)?
            }
            "benchmarks.proto3.GoogleMessage1" => {
                benchmark_dataset::<proto3::GoogleMessage1>(&mut criterion, dataset)?
            }

            /*
             TODO: groups are not yet supported
            "benchmarks.proto2.GoogleMessage2" => benchmark_dataset::<proto2::GoogleMessage2>(&mut criterion, dataset)?,
            "benchmarks.google_message3.GoogleMessage3" => benchmark_dataset::<GoogleMessage3>(&mut criterion, dataset)?,
            "benchmarks.google_message4.GoogleMessage4" => benchmark_dataset::<GoogleMessage4>(&mut criterion, dataset)?,
            */
            "benchmarks.proto2.GoogleMessage2" => (),
            "benchmarks.google_message3.GoogleMessage3" => (),
            "benchmarks.google_message4.GoogleMessage4" => (),

            _ => bail!("unknown dataset message type: {}", dataset.message_name),
        }
    }

    criterion.final_summary();
    Ok(())
}

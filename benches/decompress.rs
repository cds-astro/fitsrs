use criterion::{criterion_group, criterion_main, Criterion};
use fitsrs::Pixels;

fn criterion_benchmark_decompression(c: &mut Criterion) {
    let mut group = c.benchmark_group("decompression");
    let filenames = &[
        "samples/fits.gsfc.nasa.gov/m13real_rice.fits",
        "samples/fits.gsfc.nasa.gov/m13_rice.fits",
        "samples/fits.gsfc.nasa.gov/m13_gzip.fits",
    ];

    group.bench_function("original file m13.fits".to_string(), |b| b.iter(read_image));

    for filename in filenames {
        group.bench_function(format!("decompress {filename:?}"), |b| {
            b.iter(|| decompress(filename))
        });
    }

    group.finish();
}

fn decompress(filename: &str) {
    use fitsrs::Fits;
    use fitsrs::HDU;
    use std::fs::File;

    let f = File::open(filename).unwrap();
    let reader = std::io::BufReader::new(f);

    let mut hdu_list = Fits::from_reader(reader);

    while let Some(Ok(hdu)) = hdu_list.next() {
        if let HDU::XBinaryTable(hdu) = hdu {
            let width = hdu
                .get_header()
                .get_parsed::<i64>("ZNAXIS1")
                .unwrap()
                .unwrap() as u32;
            let height = hdu
                .get_header()
                .get_parsed::<i64>("ZNAXIS2")
                .unwrap()
                .unwrap() as u32;
            let pixels = hdu_list.get_data(&hdu).collect::<Vec<_>>();

            assert!(width * height == pixels.len() as u32);
        }
    }
}

fn read_image() {
    use fitsrs::Fits;
    use fitsrs::HDU;
    use std::fs::File;

    let f = File::open("samples/fits.gsfc.nasa.gov/m13.fits").unwrap();
    let reader = std::io::BufReader::new(f);

    let mut hdu_list = Fits::from_reader(reader);

    while let Some(Ok(hdu)) = hdu_list.next() {
        match hdu {
            HDU::Primary(hdu) | HDU::XImage(hdu) => {
                let width = hdu
                    .get_header()
                    .get_parsed::<i64>("NAXIS1")
                    .unwrap()
                    .unwrap() as u32;
                let height = hdu
                    .get_header()
                    .get_parsed::<i64>("NAXIS2")
                    .unwrap()
                    .unwrap() as u32;
                let pixels = match hdu_list.get_data(&hdu).pixels() {
                    Pixels::I16(it) => it.collect::<Vec<_>>(),
                    _ => unreachable!(),
                };

                assert!(width * height == pixels.len() as u32);
            }
            _ => (),
        }
    }
}

criterion_group!(benches, criterion_benchmark_decompression);
criterion_main!(benches);

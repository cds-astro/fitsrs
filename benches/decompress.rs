use criterion::{criterion_group, criterion_main, Criterion};
use fitsrs::hdu::data::bintable::{self, data::BinaryTableData};

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
            let width = hdu.get_header().get_parsed::<usize>("ZNAXIS1").unwrap();
            let height = hdu.get_header().get_parsed::<usize>("ZNAXIS2").unwrap();

            match hdu_list.get_data(&hdu) {
                BinaryTableData::TileCompressed(bintable::tile_compressed::pixels::Pixels::U8(
                    pixels,
                )) => assert!(width * height == pixels.count()),
                BinaryTableData::TileCompressed(
                    bintable::tile_compressed::pixels::Pixels::I16(pixels),
                ) => assert!(width * height == pixels.count()),
                BinaryTableData::TileCompressed(
                    bintable::tile_compressed::pixels::Pixels::I32(pixels),
                ) => assert!(width * height == pixels.count()),
                BinaryTableData::TileCompressed(
                    bintable::tile_compressed::pixels::Pixels::F32(pixels),
                ) => assert!(width * height == pixels.count()),
                _ => unreachable!(),
            }
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
                let width = hdu.get_header().get_parsed::<usize>("NAXIS1").unwrap();
                let height = hdu.get_header().get_parsed::<usize>("NAXIS2").unwrap();
                let pixels = match hdu_list.get_data(&hdu).pixels() {
                    fitsrs::hdu::data::image::Pixels::I16(it) => it.count(),
                    _ => unreachable!(),
                };

                assert!(width * height == pixels);
            }
            _ => (),
        }
    }
}

criterion_group!(benches, criterion_benchmark_decompression);
criterion_main!(benches);

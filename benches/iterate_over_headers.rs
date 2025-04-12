use criterion::{criterion_group, criterion_main, Criterion};

use fitsrs::FITSFile;

fn open_headers(filename: &str) {
    let hdu_list = FITSFile::open(filename).expect("Can find fits file");

    let mut corrupted = false;
    for hdu in hdu_list {
        if hdu.is_err() {
            corrupted = true;
        }
    }

    assert!(!corrupted);
}

fn criterion_benchmark_parse_only_headers(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse only headers");
    group.measurement_time(std::time::Duration::from_millis(100));

    let filenames = &[
        "samples/hipsgen/Npix8.fits",
        "samples/hipsgen/Npix9.fits",
        "samples/hipsgen/Npix132.fits",
        "samples/hipsgen/Npix133.fits",
        "samples/hipsgen/Npix134.fits",
        "samples/hipsgen/Npix140.fits",
        "samples/hipsgen/Npix208.fits",
        "samples/hipsgen/Npix282.fits",
        "samples/hipsgen/Npix4906.fits",
        "samples/hipsgen/Npix691539.fits",
        "samples/hips2fits/allsky_panstarrs.fits",
        "samples/hips2fits/cutout-CDS_P_HST_PHAT_F475W.fits",
        "samples/fits.gsfc.nasa.gov/EUVE.fits",
        "samples/fits.gsfc.nasa.gov/HST_FGS.fits",
        "samples/fits.gsfc.nasa.gov/HST_FOC.fits",
        "samples/fits.gsfc.nasa.gov/HST_FOS.fits",
        "samples/fits.gsfc.nasa.gov/HST_HRS.fits",
        "samples/fits.gsfc.nasa.gov/HST_NICMOS.fits",
        "samples/fits.gsfc.nasa.gov/HST_WFPC_II.fits",
        "samples/fits.gsfc.nasa.gov/HST_WFPC_II_bis.fits",
        "samples/vizier/NVSSJ235137-362632r.fits",
        "samples/vizier/VAR.358.R.fits",
        "samples/fits.gsfc.nasa.gov/IUE_LWP.fits",
        "samples/misc/bonn.fits",
        "samples/misc/EUC_MER_MOSAIC-VIS-FLAG_TILE100158585-1EC1C5_20221211T132329.822037Z_00.00.fits",
        "samples/misc/P122_49.fits",
        "samples/misc/skv1678175163788.fits",
        "samples/misc/SN2923fxjA.fits"
    ];
    for filename in filenames {
        group.bench_function(format!("open {:?}", filename), |b| {
            b.iter(|| open_headers(filename))
        });
    }

    group.finish();
}

criterion_group!(benches, criterion_benchmark_parse_only_headers);

criterion_main!(benches);

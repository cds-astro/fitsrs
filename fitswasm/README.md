# FITS reader written in pure Rust using [nom](https://github.com/Geal/nom)

## Install with npm

```npm i fitswasm```

## Example

```js
import * as fitswasm from "fitswasm";
// Fetch the fits file
fetch('http://alasky.u-strasbg.fr/Pan-STARRS/DR1/g/Norder3/Allsky.fits')
    // Convert to blob
    .then((response) => response.blob())
    // Get an array buffer from it
    .then((blob) => blob.arrayBuffer())
    .then((buf) => {
        // Get a bytes buffer
        const data = new Uint8Array(buf);
        // Call the wasm parser
        const fits = fitswasm.read(data));
        // fits is an js object containing a "header" key
        // and a "data" key
        const header = fits.header;
        const data = fits.data;
        // The data can be either a
        // Uint8Array, Int16Array, Int32Array,
        // Float32Array or Float64Array
        // depending on the bitpix keyword found
    })
```

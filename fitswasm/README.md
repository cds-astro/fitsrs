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
        console.log(fitswasm.readPrimaryHDUData(data));
        // The result can be a
        // Uint8Array, Int16Array, Int32Array,
        // Int64Array, Float32Array, Float64Array
        // depending on the bitpix keyword found
    })
```

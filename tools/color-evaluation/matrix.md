# Color conversion evaluation matrix

Order everywhere: current / palette 0.7.6 / color 0.3.3.

## Raw numeric comparison (native)

| Operation | Samples | Palette bitwise changes | Color bitwise changes | Palette maximum absolute channel deltas | Color maximum absolute channel deltas |
| --- | ---: | ---: | ---: | --- | --- |
| xyz | 4096 | 4095 | 4095 | [6.287710334396124e-05, 3.21576637849208e-05, 0.00022775000000008205] | [1.590432053832913e-07, 1.5700867794787854e-07, 1.8125086764797516e-07] |
| lab | 4096 | 4095 | 4095 | [0.0011526868350415498, 0.025686521653966565, 0.009091922525726659] | [1.2967628777005302e-05, 0.013070190067576704, 0.008514683577498516] |
| oklab | 4096 | 4095 | 1119 | [1.4188632257994271e-07, 3.1333937394739664e-07, 1.269965603789558e-07] | [1.1920928955078125e-07, 3.5762786865234375e-07, 1.1920928955078125e-07] |
| decode | 4096 | 3540 | 4087 | [0.9210689361396909, 4.799046989982135, 0.9221953816508706] | [1.8209324679219208e-06, 1.7624219195155888e-06, 1.9852298311917593e-06] |
| encode | 4096 | 2903 | 4090 | [11.866787959459488, 24.486743953850613, 11.918017912003645] | [1.5893104055031415e-07, 1.5585306512022612e-07, 1.5167847811081714e-07] |
| d50-d65 | 4096 | 4096 | 4096 | [0.00039344224597748223, 0.0002432006065147263, 0.0009489753182112537] | [5.324927703220794e-07, 7.082982069750443e-07, 6.139656849590835e-07] |
| d65-d50 | 4096 | 4096 | 4096 | [0.0004036345630451521, 0.00023391000473649282, 0.0005321889648033373] | [6.36169864787206e-07, 6.034775519303537e-07, 2.820257205549126e-07] |
| lab-inverse | 4096 | 4096 | 4096 | [15.56803026814176, 7.76286574790138, 2.8295717798762627] | [0.00407308594342929, 0.001798156994742145, 0.00026794924510807963] |
| oklab-inverse | 4096 | 4096 | 3955 | [0.00015603376846229366, 0.00011621404551931391, 0.00020786567654340615] | [165.31607484817505, 352.4034729003906, 1041.2447319030762] |

## Native/browser-WASM equality

| Section | Exact equality |
| --- | --- |
| artifacts | True |
| css | True |
| half_byte | True |
| lab_sweep | True |
| numeric | False |

Numeric stream equality by implementation (all input/output binary64 bits):

| Operation | Current | Palette | Color |
| --- | --- | --- | --- |
| xyz | True | True | True |
| lab | True | True | True |
| oklab | False | True | True |
| decode | True | True | True |
| encode | True | True | True |
| d50-d65 | True | True | True |
| d65-d50 | True | True | True |
| lab-inverse | True | True | True |
| oklab-inverse | False | True | True |

## Canonical RGBA changes

| Corpus | Samples | Palette | Color |
| --- | ---: | ---: | ---: |
| CSS cases | 133 | 16 | 1 |
| half_byte | 1530 | 0 | 460 |
| lab_sweep | 4096 | 2836 | 61 |

Every differing fixed CSS case (exact RGBA8, no tolerance):

| CSS | Current | Palette | Color |
| --- | --- | --- | --- |
| `color(display-p3 -0.5 0.2 1.2)` | [0, 59, 255, 255] | [0, 53, 255, 255] | [0, 59, 255, 255] |
| `color(xyz-d65 0.04045 0.0031308 0.5)` | [0, 0, 192, 255] | [0, 0, 193, 255] | [0, 0, 192, 255] |
| `color-mix(in a98-rgb, red, blue)` | [129, 0, 129, 255] | [129, 0, 128, 255] | [129, 0, 128, 255] |
| `color-mix(in rec2020, red, blue)` | [162, 19, 147, 255] | [162, 19, 148, 255] | [162, 19, 147, 255] |
| `color-mix(in lab, red, blue)` | [191, 0, 135, 255] | [189, 0, 134, 255] | [191, 0, 135, 255] |
| `color-mix(in lch, red, blue)` | [239, 0, 134, 255] | [203, 0, 126, 255] | [239, 0, 134, 255] |
| `lab(50 125 -125 / 0)` | [188, 113, 255, 0] | [183, 88, 255, 0] | [188, 113, 255, 0] |
| `lab(50 125 -125 / 0.0019607842)` | [188, 113, 255, 0] | [183, 88, 255, 0] | [188, 113, 255, 0] |
| `lab(50 125 -125 / 0.0019607844)` | [188, 113, 255, 1] | [183, 88, 255, 1] | [188, 113, 255, 1] |
| `lab(50 125 -125 / 0.5)` | [188, 113, 255, 128] | [183, 88, 255, 128] | [188, 113, 255, 128] |
| `lab(50 125 -125 / 1)` | [188, 113, 255, 255] | [183, 88, 255, 255] | [188, 113, 255, 255] |
| `lab(50 -125 125 / 0)` | [0, 151, 0, 0] | [0, 133, 48, 0] | [0, 151, 0, 0] |
| `lab(50 -125 125 / 0.0019607842)` | [0, 151, 0, 0] | [0, 133, 48, 0] | [0, 151, 0, 0] |
| `lab(50 -125 125 / 0.0019607844)` | [0, 151, 0, 1] | [0, 133, 48, 1] | [0, 151, 0, 1] |
| `lab(50 -125 125 / 0.5)` | [0, 151, 0, 128] | [0, 133, 48, 128] | [0, 151, 0, 128] |
| `lab(50 -125 125 / 1)` | [0, 151, 0, 255] | [0, 133, 48, 255] | [0, 151, 0, 255] |

## Canonical-boundary swatch artifacts

| Format | Samples (light + dark) | Palette changed bytes | Color changed bytes |
| --- | ---: | ---: | ---: |
| Svg | 40 | 34 | 6 |
| Png | 40 | 20 | 4 |
| Pdf | 40 | 34 | 6 |

New registry packages relative to production: by_address 1.2.1, color 0.3.3, fast-srgb8 1.0.0, palette 0.7.6, palette_derive 0.7.6.

## Supplemental pinned Chromium reference

Chrome/ChromeDriver 149.0.7827.55; sRGB canvas and SVG image raster, light mode.
These are browser probes, not approved Excalidraw reference exports or full parity judgments.

| CSS | Current RGBA | Palette RGBA | Color RGBA | Browser canvas pixel | Browser SVG pixel |
| --- | --- | --- | --- | --- | --- |
| `color(display-p3 -0.5 0.2 1.2)` | [0, 59, 255, 255] | [0, 53, 255, 255] | [0, 59, 255, 255] | [0, 59, 255, 255] | [0, 59, 255, 255] |
| `color(xyz-d65 0.04045 0.0031308 0.5)` | [0, 0, 192, 255] | [0, 0, 193, 255] | [0, 0, 192, 255] | [0, 0, 192, 255] | [0, 0, 192, 255] |
| `color-mix(in a98-rgb, red, blue)` | [129, 0, 129, 255] | [129, 0, 128, 255] | [129, 0, 128, 255] | [128, 0, 128, 255] | [128, 0, 128, 255] |
| `color-mix(in lab, red, blue)` | [191, 0, 135, 255] | [189, 0, 134, 255] | [191, 0, 135, 255] | [193, 0, 136, 255] | [193, 0, 136, 255] |
| `color-mix(in lch, red, blue)` | [239, 0, 134, 255] | [203, 0, 126, 255] | [239, 0, 134, 255] | [245, 0, 134, 255] | [245, 0, 134, 255] |
| `color-mix(in oklab, red, blue)` | [140, 83, 162, 255] | [140, 83, 162, 255] | [140, 83, 162, 255] | [140, 83, 162, 255] | [140, 83, 162, 255] |
| `color-mix(in rec2020, red, blue)` | [162, 19, 147, 255] | [162, 19, 148, 255] | [162, 19, 147, 255] | [162, 19, 147, 255] | [162, 19, 147, 255] |
| `color-mix(in srgb-linear, black 99.98482365082256%, white)` | [0, 0, 0, 255] | [0, 0, 0, 255] | [1, 1, 1, 255] | [1, 1, 1, 255] | [1, 1, 1, 255] |
| `lab(50 -125 125 / 0)` | [0, 151, 0, 0] | [0, 133, 48, 0] | [0, 151, 0, 0] | [0, 0, 0, 0] | [0, 0, 0, 0] |
| `lab(50 -125 125 / 0.0019607842)` | [0, 151, 0, 0] | [0, 133, 48, 0] | [0, 151, 0, 0] | [0, 0, 0, 0] | [0, 0, 0, 0] |
| `lab(50 -125 125 / 0.0019607844)` | [0, 151, 0, 1] | [0, 133, 48, 1] | [0, 151, 0, 1] | [0, 0, 0, 1] | [0, 0, 0, 1] |
| `lab(50 -125 125 / 0.5)` | [0, 151, 0, 128] | [0, 133, 48, 128] | [0, 151, 0, 128] | [0, 153, 0, 128] | [0, 153, 0, 128] |
| `lab(50 -125 125 / 1)` | [0, 151, 0, 255] | [0, 133, 48, 255] | [0, 151, 0, 255] | [0, 154, 0, 255] | [0, 154, 0, 255] |
| `lab(50 125 -125 / 0)` | [188, 113, 255, 0] | [183, 88, 255, 0] | [188, 113, 255, 0] | [0, 0, 0, 0] | [0, 0, 0, 0] |
| `lab(50 125 -125 / 0.0019607842)` | [188, 113, 255, 0] | [183, 88, 255, 0] | [188, 113, 255, 0] | [0, 0, 0, 0] | [0, 0, 0, 0] |
| `lab(50 125 -125 / 0.0019607844)` | [188, 113, 255, 1] | [183, 88, 255, 1] | [188, 113, 255, 1] | [0, 0, 0, 0] | [0, 0, 0, 0] |
| `lab(50 125 -125 / 0.5)` | [188, 113, 255, 128] | [183, 88, 255, 128] | [188, 113, 255, 128] | [213, 0, 255, 128] | [213, 0, 255, 128] |
| `lab(50 125 -125 / 1)` | [188, 113, 255, 255] | [183, 88, 255, 255] | [188, 113, 255, 255] | [214, 0, 255, 255] | [214, 0, 255, 255] |
| `lab(57.068703370168805 -25.89630411239341 105.43433617567644 / 0.6023601510096341)` | [119, 147, 0, 154] | [120, 146, 0, 154] | [119, 147, 0, 154] | [118, 147, 0, 154] | [118, 147, 0, 154] |
| `lab(75.42136115953326 -33.40447955997661 72.98849639482796 / 0.01893811230547726)` | [151, 201, 6, 5] | [152, 201, 6, 5] | [152, 201, 6, 5] | [153, 204, 0, 5] | [153, 204, 0, 5] |

Canvas/SVG pixels differ on 0/132 unique reference probes.

These comparisons retain exact channels. At low alpha, raster unpremultiplication
can differ from canonical unpremultiplied RGB; inspect the swatch PNG center
pixels in the observation JSON instead of treating this table as a tolerance gate.

## Source-line accounting

Seven replaced functions: 91 original lines (including signatures).

| Function | Original lines |
| --- | ---: |
| `decode_srgb` | 7 |
| `encode_srgb` | 7 |
| `adapt_d50_to_d65` | 10 |
| `adapt_d65_to_d50` | 10 |
| `srgb_to_xyz_d65` | 10 |
| `srgb_to_lab` | 16 |
| `canonical_lab_color` | 31 |

Original non-test color module: 1083 lines.
Shared two-candidate adapter: 85 lines.

Also replaced: color()'s XYZ-to-linear-sRGB matrix block and rectangular Oklab
mix conversion arms. Wide-RGB matrices/transfers, polar conversions, CSS admission,
missing components, alpha, gamut mapping, mix arithmetic and final quantization remain.

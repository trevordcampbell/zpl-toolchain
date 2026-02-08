# List of Existing ZPL design / render / viewer converter tools and libraries

## [cod3monk/zpl](https://github.com/cod3monk/zpl)

`Python ZPL2 Library` generates ZPL2 code which can be sent to Zebra or similar label printers. The library uses only Millimeter as unit and converts them internally according to printer settings.

| Info                                       | Value                                  |
| ------------------------------------------ | -------------------------------------- |
| Primary Languages                          | Python                                 |
| Tool Type                                  | ZPL Generator, ZPL Renderer / Viewer   |
| Uses Labelary API for ZPL Code Generation? | No                                     |
| Uses Labelary API for ZPL Rendering?       | Yes                                    |
| Actively Developed?                        | Sort of (occasional feature PR merges) |
| Actively Maintained?                       | Sort of (Last updated September 2025)  |
| Dependencies                               | Minimal                                |
| License                                    | AGPL-3.0                               |
| "Quality" Score                            | C                                      |

#### Core Features:

- Uses only Millimeter units and converts them according to printer settings

- Can handle images

#### Most Interesting Features:

- Compression utility function (mostly for large images)

---

## [DanieLeeuwner/JSZPL](https://github.com/DanieLeeuwner/JSZPL)

Generate ZPL II code from JavaScript

| Info                                       | Value                                |
| ------------------------------------------ | ------------------------------------ |
| Primary Languages                          | Javascript                           |
| Tool Type                                  | ZPL Generator, ZPL Renderer / Viewer |
| Uses Labelary API for ZPL Code Generation? | No                                   |
| Uses Labelary API for ZPL Rendering?       | No                                   |
| Actively Developed?                        | No                                   |
| Actively Maintained?                       | Sort of (Last updated April 2024)    |
| Dependencies                               | Moderate                             |
| License                                    | GPL-3.0                              |
| "Quality" Score                            | A                                    |

#### Core Features of this Tool:

- Nice readable / user-friendly usage API

- Seems to have pretty broad ZPL support

#### Most Interesting Features:

- Has a nice implementation surface of various ZPL elements.
- For unsupported elements, supports RAW ZPL via a Raw component
- Nice Layout / Grid Support

#### Important Notes:

- Incomplete implementation of ZPL II Standard

- Supports font families

- Maybe not actively maintained

---

## [mrothenbuecher/zpl-rest](https://github.com/mrothenbuecher/zpl-rest)

REST-API / frontend to send ZPL / ZPL II to a zebra label printer

| Info                                       | Value                                                         |
| ------------------------------------------ | ------------------------------------------------------------- |
| Primary Languages                          | Embedded Javascript (EJS), Javascript                         |
| Tool Type                                  | ZPL Renderer / Viewer, ZPL Print Service                      |
| Uses Labelary API for ZPL Code Generation? | No                                                            |
| Uses Labelary API for ZPL Rendering?       | Yes                                                           |
| Actively Developed?                        | No                                                            |
| Actively Maintained?                       | Sort of (very old original codebase, Last updated April 2025) |
| Dependencies                               | Moderate                                                      |
| License                                    | MIT                                                           |
| "Quality" Score                            | A                                                             |

#### Core Features of this Tool:

- REST-API to manage labels (written in ZPL), printer and to print these labels

- a simple graphical user interface for this REST-API

- you can use mustache in your ZPL-Code

- you can preview the result of your ZPL-Code

- you can test print your ZPL-Code

- you can review and reprint print jobs

- you can use placeholder in your ZPL labels `${varname}`which will be replaced through the API

#### Most Interesting Features:

- Can use placeholders / template literals in ZPL code
- Full Management GUI / Application

#### Important Notes:

- Lacks a designer or abstracted ZPL generator

---

## [bbulpett/zebra-zpl](https://github.com/bbulpett/zebra-zpl)

`Zebra::Zpl` offers a Ruby DSL to design and print labels using the ZPL programming language.

| Info                                       | Value                                                     |
| ------------------------------------------ | --------------------------------------------------------- |
| Primary Languages                          | Ruby                                                      |
| Tool Type                                  | ZPL Generator, ZPL Renderer / Viewer, ZPL Print Service   |
| Uses Labelary API for ZPL Code Generation? | No                                                        |
| Uses Labelary API for ZPL Rendering?       | No                                                        |
| Actively Developed?                        | No                                                        |
| Actively Maintained?                       | No (very old original codebase, Last updated 3 years ago) |
| Dependencies                               | Moderate                                                  |
| License                                    | MIT                                                       |
| "Quality" Score                            | A                                                         |

#### Core Features of this Tool:

- Create Labels declaratively

- Print Labels

#### Most Interesting Features:

- Allows for generating a variety of data matrices with error correction levels
- Elements can be justified and rotated on a Layout
- Image Handling + modification using a different `imag2zpl` gem, but can also be additionally processed via an `ImageMagick` gem

---

## [metafloor/zpl-image](https://github.com/metafloor/zpl-image)

A pure javascript module that converts images to either Z64-encoded or ACS-encoded GRF bitmaps for use with ZPL. The term ACS (Alternative Compression Scheme) denotes the run-length compression algorithm described in the section of the ZPL Reference Manual titled "Alternative Data Compression Scheme". Z64 typically gives better compression but is not available on all printers (especially older ones). The ACS encoding should work on any printer made since the mid 90s, maybe earlier.

| Info                                       | Value                             |
| ------------------------------------------ | --------------------------------- |
| Primary Languages                          | Javascript                        |
| Tool Type                                  | Z64 / ZPL Generator (images only) |
| Uses Labelary API for ZPL Code Generation? | No                                |
| Uses Labelary API for ZPL Rendering?       | No                                |
| Actively Developed?                        | No                                |
| Actively Maintained?                       | Sort of (Last updated July 2024)  |
| Dependencies                               | Minimal                           |
| License                                    | MIT                               |
| "Quality" Score                            | B                                 |

#### Core Features of this Tool:

- Works in both node.js and modern browsers.

- Converts the image to grayscale, then applies a user-supplied blackness threshold to decide which pixels are black.

- Optionally removes any empty/white space around the edges of the image.

- Optionally rotates the image to one of the orthogonal angles. This step is often necessary as ZPL does not provide the ability to rotate an image during formatting.

- Converts the monochrome image to a GRF bitmap.

- Converts the GRF bitmap to either Z64 or ACS encoding.

- For Z64, zlib in node.js or pako.js in the browser is used for compression.

#### Most Interesting Features:

- Can handle both Z64 + ACS code generation

---

## [SimonWaldherr/zplgfa](https://github.com/SimonWaldherr/zplgfa)

The `ZPLGFA Golang` package implements some functions to convert PNG, JPEG and GIF encoded graphic files to ZPL compatible ^GF-elements ([Graphic Fields](https://www.zebra.com/us/en/support-downloads/knowledge-articles/gf-graphic-field-zpl-command.html)).

If you need a ready to use application and don't want to hassle around with source code, take a look at the [ZPLGFA CLI Tool](https://github.com/SimonWaldherr/zplgfa/tree/master/cmd/zplgfa) which is based on this package.

| Info                                       | Value                             |
| ------------------------------------------ | --------------------------------- |
| Primary Languages                          | Go                                |
| Tool Type                                  | Z64 / ZPL Generator (images only) |
| Uses Labelary API for ZPL Code Generation? | No                                |
| Uses Labelary API for ZPL Rendering?       | No                                |
| Actively Developed?                        | No                                |
| Actively Maintained?                       | Sort of (Last updated April 2025) |
| Dependencies                               | Minimal                           |
| License                                    | MIT                               |
| "Quality" Score                            | B                                 |

#### Core Features of this Tool:

- Convert images to ZPL-compatible `^GF` Graphic Field elements

#### Most Interesting Features:

- Can handle both Z64 + ACS code generation

---

## [sungaila/PDFtoZPL](https://github.com/sungaila/PDFtoZPL)

A .NET library to convert PDF files (and bitmaps) into ZPL commands. This .NET library is built on top of:

- [PDFium](https://pdfium.googlesource.com/pdfium/) (native PDF renderer)
- [SkiaSharp](https://github.com/mono/SkiaSharp) (cross-platform 2D graphics API)

| Info                                       | Value                                   |
| ------------------------------------------ | --------------------------------------- |
| Primary Languages                          | C# / .NET                               |
| Tool Type                                  | ZPL Generator (PDF / bitmap files only) |
| Uses Labelary API for ZPL Code Generation? | No                                      |
| Uses Labelary API for ZPL Rendering?       | No                                      |
| Actively Developed?                        | Yes                                     |
| Actively Maintained?                       | Yes (Last updated June 2025)            |
| Dependencies                               | Moderate                                |
| License                                    | MIT                                     |
| "Quality" Score                            | A                                       |

#### Core Features of this Tool:

- Convert PDF files / Bitmap files to ZPL-compatible `^GF` Graphic Field elements

#### Most Interesting Features:

- Converts PDF files to ZPL

- Includes a Web GUI to use the tool

---

## [michaelrsweet/lprint:  A Label Printer Application](https://github.com/michaelrsweet/lprint)

LPrint implements printing for a variety of common label and receipt printers connected via network or USB. Features include:

- A single executable handles spooling, status, and server functionality.
- Multiple printer support.
- Each printer implements an IPP Everywhere™ print service and is compatible with the driverless printing support in Android™, Chrome OS™, iOS®, Linux®, macOS®, and Windows® 10/11 clients.
- Each printer can support options such as label modes, tear-off offsets, media tracking, media offset, print darkness, resolution, roll selection, and speed.
- Each printer can directly print "raw", Apple/PWG Raster, and/or PNG files.
- Each printer automatically recovers from out-of-media, power loss, and disconnected/bad cable issues.

| Info                                       | Value                           |
| ------------------------------------------ | ------------------------------- |
| Primary Languages                          | C                               |
| Tool Type                                  | ZPL Print Service               |
| Uses Labelary API for ZPL Code Generation? | No                              |
| Uses Labelary API for ZPL Rendering?       | No                              |
| Actively Developed?                        | Yes                             |
| Actively Maintained?                       | Yes (Last updated October 2025) |
| Dependencies                               | Moderate                        |
| License                                    | Apache-2.0                      |
| "Quality" Score                            | A                               |

#### Core Features of this Tool:

- Full Fledged Printing Application
- Support for many printer brands and types, many printer languages and drivers

#### Most Interesting Features:

- Claims driverless support for most printers

- Fault-tolerant, printers automatically recover from out-of-media, power loss, and disconnected/bad cable issues.

- Broad OS support

#### Important Notes:

- Does not do any ZPL Design or Configuration – merely allows documents to be printed via ZPL (although appears to be able to handle ZPL documents as well as natively convert image documents like PNG / PDF?)

---

## [sacherjj/simple_zpl2](https://github.com/sacherjj/simple_zpl2)

Simple Project to help in building ZPL2 strings for printing barcodes with Zebra or compatible label printers.

Documentation: [https://simple-zpl2.readthedocs.io](https://simple-zpl2.readthedocs.io/).

| Info                                       | Value                         |
| ------------------------------------------ | ----------------------------- |
| Primary Languages                          | Python                        |
| Tool Type                                  | ZPL Print Service             |
| Uses Labelary API for ZPL Code Generation? | No                            |
| Uses Labelary API for ZPL Rendering?       | No                            |
| Actively Developed?                        | Sort of                       |
| Actively Maintained?                       | Yes (Last updated April 2025) |
| Dependencies                               | Minimal                       |
| License                                    | MIT                           |
| "Quality" Score                            | A                             |

#### Core Features of this Tool:

- Methods for adding ZPL2 entries in the label data
- Error handling for data entered into methods, to maintain valid ZPL data
- Using web service to render ZPL2 label as PNG for quick development
- Simple class to print to network based ZPL label printer

#### Most Interesting Features:

- Supports generating lots of data matrices

- Layout via Field Origin definition

#### Important Notes:

- Incomplete Implementation of ZPl / unsupported Commands
- No ZPL Renderer / designer

---

## [erikn69/ZplEscPrinter](https://github.com/erikn69/ZplEscPrinter)

Printer emulator for Zpl, Esc/Pos rendering engine. The emulator is based on the [labelary](http://labelary.com/service.html) web service. You can configure print density, label size and the tcp server to listen for any incoming labels.

| Info                                       | Value                           |
| ------------------------------------------ | ------------------------------- |
| Primary Languages                          | Javascript                      |
| Tool Type                                  | ZPL Print Service Emulator      |
| Uses Labelary API for ZPL Code Generation? | No                              |
| Uses Labelary API for ZPL Rendering?       | Yes                             |
| Actively Developed?                        | Yes                             |
| Actively Maintained?                       | Yes (Last updated October 2025) |
| Dependencies                               | Significant                     |
| License                                    | Not Found                       |
| "Quality" Score                            | Unknown                         |

#### Core Features of this Tool:

- Emulates printers for testing and rendering
- Electron Application

#### Most Interesting Features:

- Can emulate ZPL and Esc / Pos printers

#### Important Notes:

- No ZPL Generation, purely a printer emulator

---

## [mtking2/py-zebra-zpl](https://github.com/mtking2/py-zebra-zpl)

A Python library to design and generate printable ZPL2 code.

| Info                                       | Value                         |
| ------------------------------------------ | ----------------------------- |
| Primary Languages                          | Python                        |
| Tool Type                                  | ZPL Generator                 |
| Uses Labelary API for ZPL Code Generation? | No                            |
| Uses Labelary API for ZPL Rendering?       | Yes                           |
| Actively Developed?                        | No                            |
| Actively Maintained?                       | No (Last updated August 2020) |
| Dependencies                               | Minimal                       |
| License                                    | MIT                           |
| "Quality" Score                            | A                             |

#### Core Features of this Tool:

- Nice readable / user-friendly usage API
- Very barebones

#### Most Interesting Features:

- Nice / simple composability to the usage API

#### Important Notes:

- Appears to be a very basic implementation
- Based off the above-mentioned [bbulpett/zebra-zpl](https://github.com/bbulpett/zebra-zpl) Ruby Gem for printing Zebra labels

---

## [Tim-Maes/PrintZPL](https://github.com/Tim-Maes/PrintZPL)

This service allows you to discover Zebra printers and send/print ZPL templates by using HTTP POST requests.

| Info                                       | Value                      |
| ------------------------------------------ | -------------------------- |
| Primary Languages                          | C# / .NET                  |
| Tool Type                                  | ZPL Print Service          |
| Uses Labelary API for ZPL Code Generation? | No                         |
| Uses Labelary API for ZPL Rendering?       | No                         |
| Actively Developed?                        | Yes                        |
| Actively Maintained?                       | Yes (Last Updated XX 2025) |
| Dependencies                               | Moderate                   |
| License                                    | MIT                        |
| "Quality" Score                            | A                          |

#### Core Features of this Tool:

- Discover Zebra Printers on Network
- Send ZPL Labels to printers via HTTPS
- You can use placeholder in your ZPL labels `${varname}`which will be replaced through the API with custom delimiter
- Batch printing of ZPL labels

#### Most Interesting Features:

- Very simple API
- Can use data placeholders / template literals in ZPL code

#### Important Notes:

- Appears to be a very basic implementation
- Based off the above-mentioned [bbulpett/zebra-zpl](https://github.com/bbulpett/zebra-zpl) Ruby Gem for printing Zebra labels

---

## [miikanissi/zebrafy](https://github.com/miikanissi/zebrafy)

`Zebrafy` is a Python 3 library for converting PDF and images to and from ZPL graphic fields (`^GF`).

`Zebrafy` consists of three conversion tools:

- **ZebrafyImage** — convert an image into valid ZPL
- **ZebrafyPDF** — convert a PDF into valid ZPL
- **ZebrafyZPL** — convert valid ZPL graphic fields into images or PDF

| Info                                       | Value                                     |
| ------------------------------------------ | ----------------------------------------- |
| Primary Languages                          | Python                                    |
| Tool Type                                  | ZPL Generator (Images and PDF Files only) |
| Uses Labelary API for ZPL Code Generation? | No                                        |
| Uses Labelary API for ZPL Rendering?       | No                                        |
| Actively Developed?                        | Sort of                                   |
| Actively Maintained?                       | Sort of (Last Updated November 2024)      |
| Dependencies                               | Moderate                                  |
| License                                    | AGPL-3.0                                  |
| "Quality" Score                            | B                                         |

#### Core Features of this Tool:

- Convert Images + PDF files to ZPL
- Convert ZPL graphic fields to PDF Files

#### Most Interesting Features:

- Decent native image processing options
- Decent composable usage API

#### Important Notes:

- Does not have a ZPL Viewer / Renderer

---

## [ricebean-net/zplbox](https://github.com/ricebean-net/zplbox)

ZplBox revolutionizes ZPL label creation by allowing you to use the full web technology stack. Design your labels with **HTML, CSS, and JavaScript**, and let ZplBox handle the conversion. Your web content is rendered as a PNG and then transformed into a ZPL graphic, giving you the freedom to incorporate **images, custom fonts, rich typography, and special characters** with ease.

Beyond web content, ZplBox also features robust **PDF support**. It can convert any PDF document into a high-quality PNG using Apache PDFBox, which is then seamlessly integrated into your ZPL label.

ZplBox offers a flexible and powerful solution for generating ZPL labels, whether you use our **Self-Hosted ZPL Print Server** or our convenient **Cloud-Hosted Service**.

| Info                                       | Value                                     |
| ------------------------------------------ | ----------------------------------------- |
| Primary Languages                          | Java, Javascript                          |
| Tool Type                                  | ZPL Generator (Images and PDF Files only) |
| Uses Labelary API for ZPL Code Generation? | No                                        |
| Uses Labelary API for ZPL Rendering?       | No                                        |
| Actively Developed?                        | Yes                                       |
| Actively Maintained?                       | Yes (Last Updated October 2025)           |
| Dependencies                               | Moderate                                  |
| License                                    | AGPL-3.0                                  |
| "Quality" Score                            | A                                         |

#### Core Features of this Tool:

- Convert Web Components (HTML / CSS / Javascript) -> PNG -> ZPL
- ZPL Print Service Server via TCP

#### Most Interesting Features:

- Convert HTML / Web Components to ZPL

#### Important Notes:

- Does not have a ZPL Viewer / Renderer

---

## [Jozo132/zpl2svg](https://github.com/Jozo132/zpl2svg)

Vanilla JS implementation of a stand-alone ZPL (Zebra Programming Language) to SVG converter.

| Info                                       | Value                             |
| ------------------------------------------ | --------------------------------- |
| Primary Languages                          | Javascript                        |
| Tool Type                                  | ZPL -> SVG Generator              |
| Uses Labelary API for ZPL Code Generation? | No                                |
| Uses Labelary API for ZPL Rendering?       | No                                |
| Actively Developed?                        | Yes                               |
| Actively Maintained?                       | Yes (Last Updated September 2025) |
| Dependencies                               | Minimal                           |
| License                                    | GPL-3.0                           |
| "Quality" Score                            | A                                 |

#### Core Features of this Tool:

- Convert ZPL code -> SVG
- ? Maybe supports converting ZPL code -> PNG ?

#### Most Interesting Features:

- Convert HTML / Web Components to ZPL
- Looks very simple
- Has nice example Demo

#### Important Notes:

- Cannot currently generate ZPL code (Although stated as a planned goal to support SVG -> ZPL generation)
- Incomplete ZPL Standard implementation

---

## [Dwarf1er/openlabel](https://github.com/Dwarf1er/openlabel)

OpenLabel is a C# library designed to simplify working with **Zebra Programming Language (ZPL)** for label printing.

| Info                                       | Value                            |
| ------------------------------------------ | -------------------------------- |
| Primary Languages                          | C# / .NET                        |
| Tool Type                                  | ZPL Generator, ZPL Print Service |
| Uses Labelary API for ZPL Code Generation? | No                               |
| Uses Labelary API for ZPL Rendering?       | No                               |
| Actively Developed?                        | Yes                              |
| Actively Maintained?                       | Yes (Last Updated July 2025)     |
| Dependencies                               | Minimal                          |
| License                                    | GPL-3.0                          |
| "Quality" Score                            | B                                |

#### Core Features of this Tool:

- **Print labels** to Zebra printers over the network.
- **Scale ZPL code** to fit different printer resolutions (DPI).
- **Use a powerful templating system** to replace placeholders and handle conditional statements.

#### Most Interesting Features:

- Scaling of ZPL code for different printer resolutions

---

## [cabal95/Zemulator](https://github.com/cabal95/Zemulator)

Zemulator is an application that allows you to emulate a ZPL compatible printer. While running, the application will listen for print requests on port 9100. If your computer supports Bluetooth Low Energy in Peripheral mode, it will also act like a bluetooth ZPL printer.

| Info                                       | Value                            |
| ------------------------------------------ | -------------------------------- |
| Primary Languages                          | C# / .NET                        |
| Tool Type                                  | ZPL Generator, ZPL Print Service |
| Uses Labelary API for ZPL Code Generation? | No                               |
| Uses Labelary API for ZPL Rendering?       | Yes                              |
| Actively Developed?                        | No                               |
| Actively Maintained?                       | Sort of (Last Updated July 2024) |
| Dependencies                               | Minimal                          |
| License                                    | MIT                              |
| "Quality" Score                            | A                                |

#### Core Features of this Tool:

- Emulate a ZPL compatible printer on Port 9100 or Bluetooth
- Can adjust the emulated details of the printer, such as label size, in the settings panel
- Ships with a local application GUI

#### Most Interesting Features:

- Allows Emulation of ZPL printer via port 9100 or Bluetooth

---

## [gistia/led-zpl](https://github.com/gistia/led-zpl)

ZPL Label Editor

| Info                                       | Value                                            |
| ------------------------------------------ | ------------------------------------------------ |
| Primary Languages                          | Typescript                                       |
| Tool Type                                  | ZPL Editor, ZPL Renderer / Viewer, ZPL Generator |
| Uses Labelary API for ZPL Code Generation? | No                                               |
| Uses Labelary API for ZPL Rendering?       | No                                               |
| Actively Developed?                        | No                                               |
| Actively Maintained?                       | No (Last Updated October 2023)                   |
| Dependencies                               | Minimal                                          |
| License                                    | Unknown                                          |
| "Quality" Score                            | A                                                |

#### Core Features of this Tool:

- Create / Edit ZPL Labels in a browser-based Canvas editor

#### Most Interesting Features:

- Has a really nice browser-based ZPL Editing Canvas GUI
- Fully Typescript / Modern Frontend Approach

#### Important Notes:

- Developed by a company *Gistia Healthcare* (presumably for healthcare / laboratory-related usage)

---

## [ingridhq/zebrash](https://github.com/ingridhq/zebrash)

Library for rendering ZPL (Zebra Programming Language) files as raster images.

This library emulates subset of ZPL engine and allows you to view most of the ZPL labels that are used by carriers such as Fedex, UPS or DHL as PNGs without the need to possess physical Zebra-compatible printer. Think of [Labelary Online ZPL Viewer](https://labelary.com/viewer.html) except it is completely free for commercial use, has no API limits and can easily be self-hosted or plugged into existing Go application so you don't need to send labels with real customers information to some 3rd-party servers

| Info                                       | Value                           |
| ------------------------------------------ | ------------------------------- |
| Primary Languages                          | Go                              |
| Tool Type                                  | ZPL Renderer / Viewer           |
| Uses Labelary API for ZPL Code Generation? | No                              |
| Uses Labelary API for ZPL Rendering?       | No                              |
| Actively Developed?                        | Yes                             |
| Actively Maintained?                       | Yes (Last Updated October 2025) |
| Dependencies                               | Minimal                         |
| License                                    | MIT                             |
| "Quality" Score                            | A                               |

#### Core Features of this Tool:

- Render ZPL files as PNG files for easy rendering

---

## [Dein-Ticket-Shop/zebrash-api](https://github.com/Dein-Ticket-Shop/zebrash-api)

REST API for rendering ZPL via the [Zebrash library](https://github.com/ingridhq/zebrash)

| Info                                       | Value                           |
| ------------------------------------------ | ------------------------------- |
| Primary Languages                          | Go                              |
| Tool Type                                  | ZPL Renderer / Viewer           |
| Uses Labelary API for ZPL Code Generation? | No                              |
| Uses Labelary API for ZPL Rendering?       | No                              |
| Actively Developed?                        | Yes                             |
| Actively Maintained?                       | Yes (Last Updated October 2025) |
| Dependencies                               | Moderate                        |
| License                                    | MIT                             |
| "Quality" Score                            | A                               |

#### Core Features of this Tool:

- HTTP API endpoint for ZPL to PNG conversion (via Zebrash)
- Configurable image dimensions in millimeters and DPI resolution
- CORS support for cross-origin requests
- Docker support for easy deployment
- Health check endpoint
- Proper error handling and logging

#### Most Interesting Features:

- API-based usage of Zebrash rendering library

#### Important Notes:

- Utilizes the Zebrash library mentioned above.

---

## [script-php/Universal-ZPL-Generator](https://github.com/script-php/Universal-ZPL-Generator)

A powerful JavaScript library that converts HTML div elements to high-quality ZPL (Zebra Programming Language) code for thermal label printing. Works universally across all printer DPI settings while maintaining perfect physical dimensions.

Printing HTML content using ZPL (Zebra Programming Language) is notoriously difficult. While there are existing solutions like "QZ Tray", they come with significant drawbacks - they're a pain to build something that depends on external software, require additional installations, and create dependency management nightmares. I created this library to provide a clean, self-contained solution that eliminates these dependencies while delivering high-quality ZPL generation directly from HTML elements.

This library gives you the freedom to generate professional ZPL labels without relying on external tools or dealing with complex setup procedures.

| Info                                       | Value                                    |
| ------------------------------------------ | ---------------------------------------- |
| Primary Languages                          | Javascript                               |
| Tool Type                                  | ZPL Generator, ? ZPL Renderer / Viewer ? |
| Uses Labelary API for ZPL Code Generation? | No                                       |
| Uses Labelary API for ZPL Rendering?       | No                                       |
| Actively Developed?                        | Yes (Newly Published October 2025)       |
| Actively Maintained?                       | Yes                                      |
| Dependencies                               | Minimal                                  |
| License                                    | Permissive                               |
| "Quality" Score                            | A                                        |

#### Core Features of this Tool:

- **High Quality**: Generates 600 DPI source images for maximum text clarity
- **Universal Compatibility**: Outputs 203 DPI ZPL that works on any printer (203, 300, 600 DPI)
- **Accurate Dimensions**: Maintains exact physical size regardless of printer DPI
- **Easy Integration**: Simple API with Promise-based async methods
- **Flexible Options**: Configurable quality, callbacks, and preview options
- **Validation**: Built-in div validation and error handling
- **Batch Processing**: Generate ZPL for multiple labels at once

#### Most Interesting Features:

- Generating ZPL code from HTML elements.
- Batch Label generation
- Auto DPI scaling
- Label / ZPL Validation

#### Important Notes:

- Very new (published October 2025)
- Uncertain if it has a UI preview / render preview, some of example code in the Readme suggests it may exist)

---

## [Alex16111977/1C_Zebra](https://github.com/Alex16111977/1C_Zebra)

A professional application for creating and editing ZPL (Zebra Programming Language) labels with a graphical interface based on PySide6.

| Info                                       | Value                                                              |
| ------------------------------------------ | ------------------------------------------------------------------ |
| Primary Languages                          | Python                                                             |
| Tool Type                                  | ZPL Editor, ZPL Renderer / Viewer, ZPL Generator                   |
| Uses Labelary API for ZPL Code Generation? | Probably                                                           |
| Uses Labelary API for ZPL Rendering?       | Yes                                                                |
| Actively Developed?                        | Yes (Newly Published October 2025)                                 |
| Actively Maintained?                       | Yes                                                                |
| Dependencies                               | Moderate                                                           |
| License                                    | Unknown / ? Restrictive ? / ? Not meant to be publicly available ? |
| "Quality" Score                            | Unknown                                                            |

#### Core Features of this Tool:

- Graphical label editor with an intuitive interface
- Element support:
  - Text fields with customizable fonts and sizes
  - Barcodes: EAN-13, Code 128, QR Code
  - Images and graphic elements
- ZPL generation with automatic conversion to code for Zebra printers
  Preview via Labelary API
- Measurement systems (mm, inches, points)
- Templates and placeholders for variable data
- Canvas Features
- Cursor Tracking - cursor position tracking with coordinates
- Zoom to Point - zoom to a specified point
- Snap to Grid - snap to a grid for precise positioning
- Element Bounds - highlight the boundaries of selected elements
- Keyboard Shortcuts - hotkeys for quick work
- Context Menu - context menu for element manipulation
- Smart Guides - smart alignment guides
- Undo/Redo - Undo and Redo actions
- Multi-Select - multiple element selection

#### Most Interesting Features:

- Probably a decent canvas editor GUI

#### Important Notes:

- Lots of the Code seems to be written in Russian 😅
- Very Newly published (October 2025)

---

## [SmolSoftBoi/node-zpl](https://github.com/SmolSoftBoi/node-zpl)

Build, tweak and render individual labels.

| Info                                       | Value                                    |
| ------------------------------------------ | ---------------------------------------- |
| Primary Languages                          | Typescript                               |
| Tool Type                                  | ZPL Generator, ? ZPL Renderer / Viewer ? |
| Uses Labelary API for ZPL Code Generation? | No                                       |
| Uses Labelary API for ZPL Rendering?       | No                                       |
| Actively Developed?                        | No                                       |
| Actively Maintained?                       | Yes (Last Updated October 2025)          |
| Dependencies                               | Minimal                                  |
| License                                    | Unknown                                  |
| "Quality" Score                            | B                                        |

#### Core Features of this Tool:

- Generate ZPL in Typescript / Node

#### Most Interesting Features:

- Very straightforward usage API... but probably needed better UX. Not super user friendly

---

## [Fabrizz/zpl-renderer-js](https://github.com/Fabrizz/zpl-renderer-js)

Two tools:

1. `ZPL-Renderer-JS` Convert Zebra ZPL labels to PNG directly in the browser (or node) without the use of third party services like Labelary or labelzoom!

2. `^XA Web Viewer` Has ZPL completions / recommendations, and lets you export ZPL in various image types (ZPL file, PNG file, PDF file)

| Info                                       | Value                             |
| ------------------------------------------ | --------------------------------- |
| Primary Languages                          | Typescript, Javascript, Go        |
| Tool Type                                  | ZPL Editor, ZPL Renderer / Viewer |
| Uses Labelary API for ZPL Code Generation? | No                                |
| Uses Labelary API for ZPL Rendering?       | No                                |
| Actively Developed?                        | Yes                               |
| Actively Maintained?                       | Yes (Last Updated October 2025)   |
| Dependencies                               | Minimal                           |
| License                                    | Unknown                           |
| "Quality" Score                            | A                                 |

#### Core Features of this Tool:

- Converts ZPL to PNG / PDF (via Zebrash)
- Edit ZPL code (with completions / suggestions)
- Send 

#### Most Interesting Features:

- The ^XA Web Viewer is a really nice web GUI for ZPL viewing, editing, and rendering. Really nice UI.
- ZPL completions in code editor are really nice (but could maybe be even better / more explanatory without hovering over options) – Field explainer at the top of the code editor is really nice as well)
- Planned to allow sending labels to printers which have Zebra Browser Print installed and running on the computer via SDK.

#### Important Notes:

- Very newly published (August 2025)

- ZPL-Renderer-JS is a wrapper of the above mentioned [ingridhq/zebrash](https://github.com/ingridhq/zebrash) Library

---

## [tomoeste/zpl-js](https://github.com/tomoeste/zpl-js)

**ZPL × JS** is for working with the ZPL II barcode label printing language in TypeScript and JavaScript. Here are the highlights:

- **Native JS ZPL parser and renderer** that runs entirely in the browser.
- **ZPL code editor for browsers** with highlighting, hover tips, and live preview (see the [Playground](https://tomoeste.github.io/zpl-js/)).
- **In-browser ZPL printer emulator** via HTTP Post.
- **React hooks** to create and print ZPL label templates with dynamic data.

Three tools:

1. [`zpl-js`](https://github.com/tomoeste/zpl-js/blob/main/packages/sdk#readme) The ZPL II parser and renderer code, along with the React hooks and provider.
2. [`zpl-js-editor`](https://github.com/tomoeste/zpl-js/blob/main/packages/editor#readme) The browser code editor with syntax highlighting and hover tips.
3. [`zpl-js-listener`](https://github.com/tomoeste/zpl-js/blob/main/packages/listener#readme) A tiny CLI tool to proxy HTTP Post print requests to a WebSocket connection. This lets you preview labels printed from another app or device!

| Info                                       | Value                                                         |
| ------------------------------------------ | ------------------------------------------------------------- |
| Primary Languages                          | Typescript                                                    |
| Tool Type                                  | ZPL Editor, ZPL Renderer / Viewer, ZPL Print Service Emulator |
| Uses Labelary API for ZPL Code Generation? | No                                                            |
| Uses Labelary API for ZPL Rendering?       | No                                                            |
| Actively Developed?                        | Maybe?                                                        |
| Actively Maintained?                       | Maybe? (Last Updated March 2025)                              |
| Dependencies                               | Minimal                                                       |
| License                                    | MIT                                                           |
| "Quality" Score                            | A                                                             |

#### Core Features of this Tool:

- Render / View ZPL code as labels
- Edit ZPL code (with completions / suggestions)
- Browser-based Printer Emulator via HTTP and small CLI tool to preview labels printed from another app or device

#### Most Interesting Features:

- Really nice web GUI for ZPL viewing, editing, and rendering. Really nice UI / UX.
- ZPL code hints in the code editor are really nice (still could be even better though)
- Create and print ZPL label templates with dynamic data
- Uses the [bwip-js](https://github.com/metafloor/bwip-js) library for barcodes, which has really broad data matrix support (even includes gs1 and other pharma-related matrices)
- Very modern, almost all typescript

#### Important Notes:

- Initially published recently (February 2025)

- Very similar function to above mentioned [Fabrizz/zpl-renderer-js](https://github.com/Fabrizz/zpl-renderer-js) library
  
  - This zpl-js library / toolset might be a little cleaner implementation than zl-renderer-js (and pretty much all typescript)

---

## [SMenigat/react-zpl-renderer](https://github.com/SMenigat/react-zpl-renderer)

Render ZPL label codes as canvas.

This builds on top of the amazing [tomoeste/zpl-js](https://github.com/tomoeste/zpl-js) library and offers a simplified component to just get raw ZPL code rendered onto your page. This repacks and internalizes `zpl-js` to simplify the usage even further.

| Info                                       | Value                           |
| ------------------------------------------ | ------------------------------- |
| Primary Languages                          | Typescript, Javascript          |
| Tool Type                                  | ZPL Renderer / Viewer           |
| Uses Labelary API for ZPL Code Generation? | No                              |
| Uses Labelary API for ZPL Rendering?       | No                              |
| Actively Developed?                        | Maybe?                          |
| Actively Maintained?                       | Maybe? (Last Updated July 2025) |
| Dependencies                               | Minimal                         |
| License                                    | Unknown                         |
| "Quality" Score                            | A                               |

#### Core Features of this Tool:

- Render / View ZPL code as labels as HTML Canvas Element

#### Most Interesting Features:

- Really nice web GUI for ZPL viewing, editing, and rendering. Really nice UI / UX.
- Very barebones renderer implementation

#### Important Notes:

- Initially published recently (July 2025)

- Built on top of the above mentioned [tomoeste/zpl-js](https://github.com/tomoeste/zpl-js) library
  
  - Aims to be a lightweight web rendering component for it

---

## [retreat896/Html2ZPL](https://github.com/retreat896/Html2ZPL)

ZPL GUI Editor V2 is a **work-in-progress web editor** for Zebra Programming Language (ZPL) labels. The goal is to emulate a "photo editor" experience for ZPL, allowing users to **design, preview, and export labels** visually. 

At the moment, this project is still under **active development** and may contain bugs or incomplete features.

| Info                                       | Value                                                          |
| ------------------------------------------ | -------------------------------------------------------------- |
| Primary Languages                          | Javascript                                                     |
| Tool Type                                  | ZPL Designer, ZPL Editor, ZPL Generator, ZPL Renderer / Viewer |
| Uses Labelary API for ZPL Code Generation? | No                                                             |
| Uses Labelary API for ZPL Rendering?       | No                                                             |
| Actively Developed?                        | Sort of (Very slow, might get abandoned)                       |
| Actively Maintained?                       | Sort of (Last Updated Roadmap September 2025)                  |
| Dependencies                               | Minimal                                                        |
| License                                    | Unknown                                                        |
| "Quality" Score                            | Not Applicable... but idea has potential                       |

#### Core Features of this Tool:

- Make a Designer / Editor to visually design and edit ZPL Labels via a Canvas GUI

#### Most Interesting Features:

- Aims to be a visual Designer / Editor for ZPL labels.

#### Important Notes:

- New project with no real velocity behind it. High likelihood of abandonment and incompletion.

---

## [C4J/ZPL-Renderer](https://github.com/C4J/ZPL-Renderer)

View Zebra ZPL on screen, save as PDF and Print to non ZPL printers

| Info                                       | Value                                    |
| ------------------------------------------ | ---------------------------------------- |
| Primary Languages                          | Java                                     |
| Tool Type                                  | ZPL Renderer / Viewer, ZPL Print Service |
| Uses Labelary API for ZPL Code Generation? | No                                       |
| Uses Labelary API for ZPL Rendering?       | No                                       |
| Actively Developed?                        | Maybe ?                                  |
| Actively Maintained?                       | Maybe ? (Last Updated August 2025)       |
| Dependencies                               | Unknown                                  |
| License                                    | GPL-2.0                                  |
| "Quality" Score                            | Not Applicable... but idea has potential |

#### Core Features of this Tool:

- ZPL rendering / Viewing on screen
- Save ZPL label as PDF
- Print to printers over network

#### Important Notes:

- New project initially published August 2025.
- Java 🤢

---

## [porrey/Virtual-ZPL-Printer](https://github.com/porrey/Virtual-ZPL-Printer)

An Ethernet based virtual Zebra Label Printer that can be used to test applications that produce bar code labels. This application uses the Labelary service found at [http://labelary.com](http://labelary.com/service.html).

| Info                                       | Value                                |
| ------------------------------------------ | ------------------------------------ |
| Primary Languages                          | C# / .NET                            |
| Tool Type                                  | ZPL Print Service Emulator           |
| Uses Labelary API For ZPL Code Generation? | No (No generation)                   |
| Uses Labelary API for ZPL Rendering?       | Yes                                  |
| Actively Developed?                        | Not really                           |
| Actively Maintained?                       | Sort of (Last Updated February 2025) |
| Dependencies                               | unknown                              |
| License                                    | LGPL-3.0                             |
| "Quality" Score                            | A                                    |

#### Core Features of this Tool:

- Emulate ZPL Printer to test labels

#### Most Interesting Features:

- Has ZPL Code Warnings in the ZPL viewer tool

---

## [neudeeptech/Zpl-Label-Generator](https://github.com/neudeeptech/Zpl-Label-Generator)

Welcome to the Label Generator Application, a user-friendly tool designed for crafting Zebra Programming Language (ZPL) labels effortlessly. This innovative application seamlessly blends the versatility of [JavaScript/ReactJS (FabricJS)] with the robustness of Python/Flask to provide a seamless label designing experience

| Info                                       | Value                           |
| ------------------------------------------ | ------------------------------- |
| Primary Languages                          | C# / .NET                       |
| Tool Type                                  | ZPL Designer, ZPL Generator     |
| Uses Labelary API For ZPL Code Generation? | No                              |
| Uses Labelary API for ZPL Rendering?       | No                              |
| Actively Developed?                        | No                              |
| Actively Maintained?                       | Sort of (Last Updated May 2024) |
| Dependencies                               | Moderate                        |
| License                                    | GPL-3.0                         |
| "Quality" Score                            | A                               |

#### Core Features of this Tool:

- Canvas Label Designer, generates ZPL
- **Feature 1:** You can add text/labels.
- **Feature 2:** You can change font size,style and weight
- **Feature 3:** You can Use different shapes, lines.
- **Feature 4:** You can change the label page size with required width and height along with units like mm, cm, inch.
- **Feature 5:** You can save your label as json and zpl file
- **Feature 6:** You can add Barcode , QRcode , company logo/Image
- **Feature 7:** You can retrieve the saved label drawing by opening the json file of label back on canvas in the editable drawing format.

#### Most Interesting Features:

- Has a Canvas Label Designer GUI. Very barebones, but appears to work.

---

## [DmytroVasin/zpl-labelary-preview - VSCode Extension](https://github.com/DmytroVasin/zpl-labelary-preview)

ZPL Labelary Preview is a VSCode extension designed to preview ZPL (Zebra Programming Language) labels using the [Labelary API](https://labelary.com/service.html). It allows users to view label renderings directly within VSCode, manipulate label dimensions and density, and download the labels in various formats such as ZPL, PNG, and PDF.

| Info                                       | Value                             |
| ------------------------------------------ | --------------------------------- |
| Primary Languages                          | Javascript                        |
| Tool Type                                  | ZPL Renderer / Viewer             |
| Uses Labelary API For ZPL Code Generation? | No                                |
| Uses Labelary API for ZPL Rendering?       | Yes                               |
| Actively Developed?                        | Not Really                        |
| Actively Maintained?                       | Sort of (Last Updated March 2025) |
| Dependencies                               | Minimal                           |
| License                                    | MIT                               |
| "Quality" Score                            | B                                 |

#### Core Features of this Tool:

- **Preview ZPL Labels**: Render and view your ZPL label designs within a VSCode panel.
- **Preview ZPL Code**: View ZPL code with syntax highlighting and prettified formatting in a separate panel.
- **Base64 Decoding**: Automatically decode ZPL labels encoded in Base64 format for rendering and preview.
- **Clipboard Copy**: Copy the raw ZPL code to the clipboard.
- **Label Size Adjustment**: Modify label dimensions dynamically (width, height).
- **Density Selection**: Change the print density (dpmm).
- **Multi-Page Support**: Navigate between multiple pages of label designs.
- **Download Options**: Save labels as ZPL, PNG, or PDF files.

#### Most Interesting Features:

- View ZPL labels in VSCode Window

- Can select sections of ZPL code in file to view / render

- Can view multiple labels on tabs in the viewer window

---

## [crbnos/zpl-print](https://github.com/crbnos/zpl-print)

ZPL Print Server – A simple web server built with Hono.js that handles ZPL print requests remotely via ngrok

| Info                                       | Value                            |
| ------------------------------------------ | -------------------------------- |
| Primary Languages                          | Typescript                       |
| Tool Type                                  | ZPL Print Service                |
| Uses Labelary API For ZPL Code Generation? | No                               |
| Uses Labelary API for ZPL Rendering?       | No                               |
| Actively Developed?                        | Not Really                       |
| Actively Maintained?                       | Sort of (Last Updated June 2025) |
| Dependencies                               | Moderate                         |
| License                                    | Apache-2.0                       |
| "Quality" Score                            | A                                |

#### Core Features of this Tool:

- Send labels to printers via API over ngrok webserver

- Printers and work centers must be defined manually in config.

#### Most Interesting Features:

- Accepts prints requests either by sending raw ZPL code over the wire, or by passing in a callback URL for wherever the ZPL code lives (theoretically this could be enhanced to allow calling a generation service as well which returns ZPL)

#### Important Notes:

- Recently Initially Published in March 2025

- Very modern, all Typescript

- Produced by CarbonOS team, who makes an ERP for manufacturing (high quality output team). Also many parallels in product requirements to Biotech / Pharma

---

---

## To-Do

- Look up which one wanted to render in 10ms

- Look up which one claims to support any font / design (probably one of the HTML / Bitmap converter ones)

---

---

## Qualities We Want in Each part

### ZPL Generator:

- Single Label Generation

- Batch Label Generation

- Broad / Complete ZPL II Support

- Able to use value variables / template literals to define injectable values

- Possibly Generate Code other than ZPL (like xx) / convert other Codes to ZPL / Transpile 

- 

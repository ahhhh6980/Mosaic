# Mosaic
*To compile:* `cargo build --release`
## Convert images into a mosaic of a subset of images!


command line args:
`./mosaic -f test.png -p emoji -fs 128 -ps 64`
- Specify *file.png* scaled to 128x128, using the palette "emoji" scaled to 64x64[^1][^2]

Pass in the extension for the file name!

## How Do You Use Other Images?
1 **Find a collection of images**
- I recommend the 3500+ emojis available from here: https://twemoji.twitter.com/

2 **Put the images into a named folder**
- In this repo, packaged for convenience, the mentioned collection is named emojis/

3 **Put named folder under palettes/**
- The name of the folder is the palette name

![](https://ninja.dog/3SVRMe.jpg)
[^1]: image size determines that scale that it processes the image
[^2]: palette size specifies what size the palette images are

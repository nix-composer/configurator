# Web app icons

One `<id>.png` per entry in `../webapps.json`, 128×128, built into the
installer. From [dashboard-icons](https://github.com/homarr-labs/dashboard-icons)
(Apache-2.0), except `hey.png`, from [Omarchy](https://github.com/basecamp/omarchy)
(MIT). The logos are their owners' trademarks.

To add one: fetch it (`https://cdn.jsdelivr.net/gh/homarr-labs/dashboard-icons/png/<name>.png`),
then `magick <id>.png -resize 128x128 -background none -gravity center -extent 128x128 <id>.png`.

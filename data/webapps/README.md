# Web app icons

One `<id>.png` per entry in `../webapps.json`, 128×128, built into the
installer. From [dashboard-icons](https://github.com/homarr-labs/dashboard-icons)
(Apache-2.0), except `hey.png`, from [Omarchy](https://github.com/basecamp/omarchy)
(MIT), and these, from [selfh.st icons](https://github.com/selfhst/icons)
(CC BY 4.0) where dashboard-icons has none: adobe-creative-cloud,
adobe-express, amazon-music, aol-mail, audible, capcut, codepen,
digitalocean, doordash, etsy, heroku, humble-bundle, imdb,
interactive-brokers, internet-archive, last-fm, letterboxd, lyft,
microsoft-clipchamp, new-york-times, nextdoor, notebooklm, openrouter,
raindrop, roblox, schwab, target, teamviewer, temu, tldraw, tradingview,
trello, uber, vimeo, walmart. The logos are their owners' trademarks.

To add one: fetch it (`https://cdn.jsdelivr.net/gh/homarr-labs/dashboard-icons/png/<name>.png`,
else `https://cdn.jsdelivr.net/gh/selfhst/icons/png/<name>.png`), then
`magick <id>.png -resize 128x128 -background none -gravity center -extent 128x128 <id>.png`.

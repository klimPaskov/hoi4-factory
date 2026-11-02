# HOI4 Factory Discord assets

The mascot of HOI4 Factory (the brick factory with a scarf and toolbelt) as a ready-to-upload asset set for the Discord server at https://discord.gg/rAXesGcT2t.

Upload this file: [hoi4-factory-discord-assets.zip](hoi4-factory-discord-assets.zip). It holds `emoji/`, `stickers/` and `server/`. Preview the whole set on [dark](contact-sheet-dark.png) and [light](contact-sheet-light.png) backgrounds.

## How to upload

Uploading is a server admin step.

1. Unzip the file.
2. Emoji: Server Settings, then Emoji, then Upload Emoji. Select all PNG and GIF files in `emoji/`. Discord names each emoji after the file, so `factory_wave.png` becomes `:factory_wave:`. A server without boosts has 50 static and 50 animated emoji slots, and this set uses 24 and 3.
3. Stickers: Server Settings, then Stickers, then Upload Sticker. Upload each file in `stickers/` one at a time. Discord asks for a name, a related emoji and a description. A server without boosts has 5 sticker slots, and more unlock with boost levels. Upload the ones you like most first, such as GG, On it and Ship it.
4. Icon: Server Settings, then Overview, then Upload Image, with `server/server-icon-1024.png`. Discord crops the icon to a circle, and the face and chimney sit inside that circle.
5. Banner: needs boost level 2. Server Settings, then Server Banner, then upload `server/server-banner-960x540.png`.

## Static emoji (24, 128x128 PNG)

Each is a tight crop of the face and upper body with a white sticker outline, so it stays readable at 22 to 32 px. The pose comes from the app's mascot set for the first group and was generated with the mascot master image as the reference for the others.

| Name | Use |
|---|---|
| `factory_wave` | Hello and goodbye |
| `factory_cheer` | Celebrate, release and milestone posts |
| `factory_thumbsup` | Approve, nice work |
| `factory_think` | Wondering, considering an idea |
| `factory_surprised` | Shock, wow |
| `factory_sorry` | Apology, my mistake |
| `factory_eyes` | Suspicious side-eye, watching a thread |
| `factory_laugh` | Laughing out loud |
| `factory_love` | Love, thanks, favourite mod |
| `factory_salute` | Respect, aye aye |
| `factory_facepalm` | A bug that should not exist |
| `factory_cry` | Sad, a crash, a lost save |
| `factory_angry` | Rage, a broken tooltip |
| `factory_sleepy` | Late night modding, a long build |
| `factory_party` | Parties, launches, new members |
| `factory_gg` | Good game, GG |
| `factory_please` | Begging for a feature or a fix |
| `factory_ok` | OK, understood, fine |
| `factory_fire` | Hype, hot take, on fire |
| `factory_popcorn` | Watching a debate with popcorn |
| `factory_hammer` | Building, fixing, working on a mod |
| `factory_medal` | Salute with a gold medal, an award or a job well done |
| `factory_map` | Planning the map, scouting, a strategy discussion |
| `factory_focus` | Holding a focus tree plaque, tree design and national focus talk |

## Animated emoji (3, 128x128 GIF)

Each GIF is built from the same pose art as the static set, with 8 frames and under 100 KB.

| Name | Use |
|---|---|
| `factory_wave_hop` | A hopping wave for greetings |
| `factory_cheer_bounce` | A bouncing cheer for celebrations |
| `factory_laugh_shake` | A shaking laugh for jokes |

## Stickers (8, 320x320 PNG)

Each is a large pose with a white outline and short text where useful. Suggested related emoji are in brackets.

| Name | Text | Use |
|---|---|---|
| `factory_gg` | GG on a sign | Good game (🎮) |
| `factory_on_it` | On it! | Taking a task (🔧) |
| `factory_ship_it` | Ship it | Approving a release (🚀) |
| `factory_hello` | Hello! | Welcoming new members (👋) |
| `factory_thanks` | Thanks! | Saying thanks (❤️) |
| `factory_my_bad` | My bad | Admitting a mistake (😅) |
| `factory_lets_build` | Let's build | Starting a project (🔨) |
| `factory_victory` | Victory! | Celebrating a win (🎉) |

## Server icon and banner

| File | Size | Notes |
|---|---|---|
| `server/server-icon-1024.png` | 1024x1024 | The mascot's face and chimney on a dark blue brick background. Readable at 48 px inside the circle crop. |
| `server/server-banner-960x540.png` | 960x540 | Title, the tagline and the three projects on a dark blue background with the mascot pointing at the title. |

## How the set was made

- The new expressions were generated with the image model that made the mascot (ChatGPT plan, no paid API key), using `.work/mascot-src/master.png` as the reference image, on a flat magenta background. The existing poses come from `.work/mascot-src`.
- `key_cutouts.py` removes the magenta (or keeps real transparency when the model returned it) and trims each pose.
- `build_emoji.py` crops, adds the outline and writes the static emoji. `build_assets.py` writes the GIFs, stickers, icon and banner. `build_contact_sheet.py` writes the previews. `check_limits.py` checks every file against Discord's limits.
- Raw generated images stay in the git-ignored `.work/discord-src`. The text uses Arial Black from the Windows font folder, so rebuilding needs Windows or a replacement font path.

## Limits check

`check_limits.py` passes on all files. The limits are 128x128 and under 256 KB for emoji, 320x320 and under 512 KB for stickers, and an icon of at least 512x512 and a banner of at least 960x540.

| Group | Count | Largest file |
|---|---|---|
| Static emoji | 24 | 29 KB |
| Animated emoji | 3 | 80 KB |
| Stickers | 8 | 107 KB |
| Icon | 1 | 711 KB |
| Banner | 1 | 258 KB |

The mascot does not carry real flags or military insignia. The medal is a generic gold star.

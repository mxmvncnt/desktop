# To-do

- overscroll bounce
- make the code cleaner for humans
  - debug mode in the app that shows unique strings in the app to find the code associated with a feature or element faster
  - add documentations for humans
- bundle as flatpak
- bundle for macos
- fixes:
  - when the commit message input is over the limit and we start typing and it scrolls automatically, it jumps up on one keypress and down on the second
  - when browsing through commits with the arrow buttons, the diff briefly flashes in black, comes back and then displays the next diff. It should only have the flash to black and immediately show the next diff.
  - similar thing happens when switching the selected commit on the sidebar. The title of the commit changes instantly, but the impacted files and diff show up a few hundred milliseconds later later

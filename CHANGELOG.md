# Changelog

## 0.8.0

- Complete visual overhaul
- JRE swapper now supports Mikohime's Java 23/24!
  - Only supports Windows and Linux, as it's still just Mikohime and that doesn't support macOS
- Multiple themes!
  - Custom themes!
- The web browser is now completely integrated *and embedded* into the app, meaning it won't open in a new window anymore
- Mods with missing or outdated dependencies cannot be enabled and will tell you which dependencies they're missing/are outdated
- Other changes I've forgotten
- Lots of fixed bugs!
- New and exciting bugs!

## 0.7.x

- Oops forgot to update this
- Anything mentioned below as not being supported on another platform should now be available on all platforms

## 0.6.0

- Complete rewrite of application in Druid.
- Feature parity achieved with 0.5.x
- **New**
  - Added new side panel: contains `Tools & Filters` and `Launch`
  - Filters
    - Filters are activated by unchecking a condition in the new `Tools & Filters` pane
    - Conditions include version checker support, update availability, semver difference between local and available version, etc
  - Toggles
    - Toggles are available in the `Tools & Filters` panel
    - Currently only two toggles: `Enable All` and `Disable All`
  - Launch
    - The `Launch` panel contains the previous launch and starsector version widgets
    - It also now contains a widget displaying the selected installation directory and a browse button for faster access.
  - Sort indicators
    - Sorting is now indicated by arrow icons indicating the direction of sorting (ascending/descending)
  - Icons
    - Icons have been utilised throughout the application to improve user experience
  - Dark Theme
    - The default (and only) theming is now dark
    - Light theme is not currently a priority
    - Your retinas can thank me later
  - (Dormant) App self updating
    - MOSS (aka: this app) will be able to update itself whenever a new version is release.
    - Only available on Windows and Linux, aka: *not supported on macOS*
- **Fix**
  - VMParams editing is now only available on Windows
    - It never worked on other platforms *anyway*
    - May be implemented for other platforms in the future

# Claude Design follow-up: wobook Android, round 2

Paste below the line into the same Claude Design project. Keep every existing
frame, token and the readme; apply these changes and add the listed frames.

---

Thanks, round 1 is complete and the token-to-Compose mapping is exactly what
the coding agent needs. Six fixes, then four new frames.

## Fixes to existing frames and components

1. **Tags may contain spaces.** Real tags in my library include `ui library`,
   `ai agent`, `text-to-diagram`. Change the `TagEditor` rule everywhere
   (Form-Fetching, Form-AlreadySaved, Share-Fresh, Share-Offline, Share-Failed,
   DS-3 note, readme) from "Space or comma adds a tag" to: **comma or Enter
   commits a chip, space is part of the tag**. Update the helper line under the
   field accordingly. Autocomplete must match across the space (`ui lib` ->
   `ui library`).

2. **Share-Saved toast must be plain text.** Android 12+ only allows text-only
   system toasts from a dismissed activity, no icon, no custom colors. Redraw
   Share-Saved with the stock Android toast shape (rounded pill, system
   surface, single line) reading `Saved to wobook · react, ui library`. Add a
   fallback variant where the confirmation is shown inside the sheet for 600 ms
   (check icon, "Saved") before the sheet dismisses, for the case where the
   toast is not available.

3. **Pair-Scan hint must be platform neutral.** Desktops have no Devices
   screen; they print the code in a terminal with `wobook pair`. Replace "On
   the other device open Devices → Show my QR" with something like "On a
   desktop run `wobook pair`. On a phone open Devices → Show my QR." Mono for
   the command.

4. **URL is the bookmark's identity.** Editing it moves the bookmark. In
   Form-AlreadySaved make the URL field read-only with a trailing "Change"
   text button; tapping it reveals an editable field plus a one-line note
   "Changing the URL replaces this bookmark; tags and description are kept."
   Add that expanded state as a small frame or an inset.

5. **Fingerprint size mismatch.** DS-4 draws it at mono 18/26, the readme says
   20/30. Pick 18/26 and align both.

6. **Keyboard-up Home with FAB.** Home-Results hides the Add FAB while typing.
   Confirm this is intended by stating it in the frame label, or show where
   Add lives while the keyboard is up (Home-NoResults already offers "Add a
   bookmark" inline, which is fine).

## New frames

7. **Light theme, Latte.** Render these four existing frames with
   `data-theme="light"`, side by side with their dark versions:
   Home-Results, Share-Fresh, Detail, Pair-Confirm. Check the accent stays a
   line and a highlight, not a fill, and that matched characters in result
   titles still read at 4.5:1.

8. **Dynamic type 130 percent.** Home-Results at font scale 1.3: result rows
   grow to min-height 64 plus, title and URL still one line each with
   ellipsis, tag line still one clipped line with `+N`, trailing buttons stay
   48 dp. Also the Share-Fresh sheet at 1.3 with the keyboard up, showing
   that the Save button stays reachable above the keyboard.

9. **Devices, own device row.** Add this device as the first row in Devices
   (name, "this device", platform icon, no menu) and remove it from the footer,
   which keeps only the sync status. Update Devices, Devices-RowMenu,
   Devices-Revoke, Pair-Success.

10. **Tablet or landscape check, one frame.** Home-Idle at 800 x 1280 dp to
    show whether the list simply widens or gets a max content width. Keep it
    to one frame; this is a phone app, I only want the rule stated.

Update `readme.md` for items 1, 2, 4, 5 and 9. No other scope changes: still
no folders, favorites, accounts or tag management screen.

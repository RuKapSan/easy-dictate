# Gemini UX/UI Analysis - Easy Dictate

**Date:** 2025-12-10
**Analyzed version:** feat/ux-improvements branch (with dual hotkey feature)

## Screenshot Analyzed
`tests/e2e/screenshots/should_have_new_hotkey_UI_elements_PASS_2025-12-10T00-27-16-608Z.png`

---

## Analysis Summary

### 1. Visual Hierarchy
- **Good:** Status section with glowing orb serves as excellent focal point
- **Bad:** Settings section looks monotonous - all fields have equal visual weight

### 2. Space Usage
- **Imbalance:** Left column (Status) has much empty space, right column (Settings) is overloaded
- **Redundancy:** "Записать" buttons take too much space for secondary function

### 3. Intuitiveness
- **Clear:** Field purposes understood via labels
- **Outdated UX:** Separate "Record" button for hotkeys is old pattern
- **Missing:** No eye icon for API key visibility toggle

---

## Concrete Recommendations

### Priority 1: Hotkey UX (Critical)
**Current:** Button "Записать" + field
**Recommended:**
- Remove "Записать" buttons completely
- Click on field = enter capture mode
- Field highlights, waits for keypress
- Add small (x) to clear hotkey on hover

### Priority 2: Panel Optimization (Layout)
- Visually separate "Technical Settings" (Provider/API/Model) from "Controls" (Hotkeys)
- Use subtle dividers or card backgrounds
- Add eye icon to API key field for show/hide

### Priority 3: Balance & Alignment
- Ensure consistent padding throughout
- Move "Minimize to tray" to window titlebar or make it a checkbox setting

### Priority 4: Interactive Orb
- Make orb clickable to start/stop dictation
- Add hover effect (slight size increase or brightness change)

### Priority 5: Visual Polish
- Increase input field height by 4-6px
- Improve label contrast (lighter gray for better readability)

---

## Proposed Layout (Text Mockup)
```
[ LEFT COLUMN ]            [ RIGHT COLUMN ]
                           --------------------------
       ( O )               Provider: [ OpenAI v ]
                           API Key:  [ •••••• (👁)]
      STATUS               Model:    [ whisper-1 v]
  Ready to record          --------------------------
                           HOTKEYS
                           Start/Stop: [  F2  ] (no Записать button)
                           Translate:  [  F3  ]
                           Toggle:     [  F4  ]
                           --------------------------
                           (o) Always on top
```

---

## Implementation Priority

1. **Remove "Записать" buttons** - click on hotkey field to capture (game-style)
2. **Group settings visually** - cards/dividers
3. **API key eye icon** - show/hide toggle
4. **Clickable orb** - duplicate hotkey functionality
5. **Visual polish** - spacing, contrast

---

*Analysis by Gemini 3 Pro via PAL skill*

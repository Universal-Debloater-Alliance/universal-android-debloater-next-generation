---
name: Update apps description or recommendation
about: You want to improve/update a description/recommendation
title: ""
labels: package::documentation
assignees: ""
---

**Your phone model**:

**Packages documentation to update:**

```
com.this.is.a.application
com.this.is.another.application
...
```

## Documentation Change

**List**: `Google`|`Misc`|`OEM` (manufacturer)|`AOSP`|`Pending`|`Carrier` (isp).

**Removal**:

- Recommended -- Pointless or outright negative packages, and/or apps available through Google Play.
- Advanced -- Breaks obscure or minor parts of functionality, or apps that aren't easily enabled/installed through Settings/Google Play. This category is also used for apps that are useful (default keyboard/gallery/launcher/music app.) but that can easily be replaced by a better alternative.
- Expert -- Breaks widespread and/or important functionality, but nothing important to the basic operation of the operating system. Removing an Expert package should not bootloop the device (unless mentioned in the description) but we can't guarantee it 100%.
- Unsafe -- Can break vital parts of the operating system. Removing an Unsafe package have an extremely high risk of bootlooping your device.

### \<package name\>

**List**: \<current list\> :arrow_right: \<proposed list\>
**Removal**: \<current recommendation list\>
:arrow_right: \<proposed recommendation list\>

### Current description

> Current description

### Proposed description

> Proposed description

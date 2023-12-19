---
name: Add new package(s)
about: You want to add new apps in the debloat list
title: ""
labels: package::addition
assignees: ""
---

**Your phone model:**

**Packages:**

```
com.this.is.a.bad.application
com.this.is.another.bad.application
...
```

- [ ] **I removed all those packages on my phone**
      If not why. Leave the brackets blank and explain why.

## Document each package the best you can

**List**: `Google`|`Misc`|`OEM` (manufacturer)|`AOSP`|`Pending`|`Carrier` (isp).

**Removal**:

- Recommended -- Pointless or outright negative packages, and/or apps available through Google Play.
- Advanced -- Breaks obscure or minor parts of functionality, or apps that aren't easily enabled/installed through Settings/Google Play. This category is also used for apps that are useful (default keyboard/gallery/launcher/music app.) but that can easily be replaced by a better alternative.
- Expert -- Breaks widespread and/or important functionality, but nothing important to the basic operation of the operating system. Removing an Expert package should not bootloop the device (unless mentioned in the description) but we can't guarantee it 100%.
- Unsafe -- Can break vital parts of the operating system. Removing an Unsafe package have an extremely high risk of bootlooping your device.

### \<package name\>

**List**: \<list\>
**Removal**: \<recommendation list\>

> Description. Link to its Playstore page if it exists.

crap.globals.define("navigation", {
  labels = { singular = "Navigation" },
  fields = {
    crap.fields.array({
      name = "main_nav",
      admin = {
        label_field = "label",
        labels = {
          singular = { en = "Nav Item", de = "Navigationselement" },
          plural = { en = "Nav Items", de = "Navigationselemente" },
        },
      },
      fields = {
        crap.fields.text({ name = "label", required = true, localized = true }),
        crap.fields.text({ name = "url", required = true, admin = { placeholder = "/about" } }),
        crap.fields.checkbox({ name = "open_in_new_tab", default_value = false }),
        crap.fields.array({
          name = "children",
          admin = { label_field = "label", labels = { singular = "Sub Item", plural = "Sub Items" } },
          fields = {
            crap.fields.text({ name = "label", required = true, localized = true }),
            crap.fields.text({ name = "url", required = true }),
          },
        }),
      },
    }),
  },
  access = {
    read = "access.anyone",
    update = "access.admin_only",
  },
})

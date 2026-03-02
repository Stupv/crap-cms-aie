crap.globals.define("site_settings", {
  labels = { singular = "Site Settings" },
  fields = {
    crap.fields.tabs({
      name = "settings_tabs",
      tabs = {
        {
          label = "General",
          fields = {
            crap.fields.text({ name = "site_name", required = true, default_value = "Meridian Studio" }),
            crap.fields.text({ name = "tagline", admin = { placeholder = "Design. Build. Launch." } }),
            crap.fields.email({ name = "contact_email" }),
            crap.fields.text({ name = "phone" }),
            crap.fields.textarea({ name = "address", admin = { rows = 3 } }),
          },
        },
        {
          label = "Branding",
          fields = {
            crap.fields.upload({ name = "logo", relationship = { collection = "media" } }),
            crap.fields.upload({ name = "favicon", relationship = { collection = "media" } }),
            crap.fields.text({ name = "primary_color", default_value = "#2563eb", admin = { placeholder = "#hex" } }),
            crap.fields.text({ name = "secondary_color", default_value = "#7c3aed", admin = { placeholder = "#hex" } }),
          },
        },
        {
          label = "Social",
          fields = {
            crap.fields.group({
              name = "social",
              fields = {
                crap.fields.text({ name = "github", admin = { placeholder = "https://github.com/meridian" } }),
                crap.fields.text({ name = "twitter", admin = { placeholder = "https://twitter.com/meridian" } }),
                crap.fields.text({
                  name = "linkedin",
                  admin = { placeholder = "https://linkedin.com/company/meridian" },
                }),
                crap.fields.text({ name = "instagram", admin = { placeholder = "https://instagram.com/meridian" } }),
                crap.fields.text({ name = "youtube", admin = { placeholder = "https://youtube.com/@meridian" } }),
              },
            }),
          },
        },
      },
    }),
  },
  access = {
    read = "access.anyone",
    update = "access.admin_only",
  },
})

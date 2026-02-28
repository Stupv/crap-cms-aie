crap.collections.define("pages", {
  labels = {
    singular = "Page",
    plural = "Pages",
  },
  timestamps = true,
  versions = true,
  admin = {
    use_as_title = "title",
    default_sort = "title",
    list_searchable_fields = { "title", "slug" },
  },
  fields = {
    {
      name = "title",
      type = "text",
      required = true,
      localized = true,
    },
    {
      name = "slug",
      type = "text",
      required = true,
      unique = true,
      localized = true,
      admin = {
        description = "URL path (e.g., 'about' for /about)",
        width = "half",
      },
      hooks = {
        before_validate = { "hooks.auto_slug" },
      },
    },
    {
      name = "content",
      type = "blocks",
      localized = true,
      blocks = {
        {
          type = "richtext",
          label = "Rich Text",
          fields = {
            { name = "body", type = "richtext" },
          },
        },
        {
          type = "image",
          label = "Image",
          label_field = "caption",
          fields = {
            {
              name = "image",
              type = "upload",
              required = true,
              relationship = { collection = "media" },
            },
            { name = "caption", type = "text" },
          },
        },
        {
          type = "cta",
          label = "Call to Action",
          label_field = "heading",
          fields = {
            { name = "heading", type = "text", required = true },
            { name = "body", type = "textarea" },
            { name = "button_text", type = "text", required = true },
            { name = "button_url", type = "text", required = true },
          },
        },
        {
          type = "deep",
          label = "Deep Nesting",
          fields = {
            {
              name = "nested",
              type = "blocks",
              blocks = {
                {
                  type = "richtext",
                  label = "Rich Text",
                  fields = {
                    { name = "body", type = "richtext" },
                  },
                },
                {
                  type = "image",
                  label = "Image",
                  label_field = "caption",
                  fields = {
                    {
                      name = "image",
                      type = "upload",
                      required = true,
                      relationship = { collection = "media" },
                    },
                    { name = "caption", type = "text" },
                  },
                },
                {
                  type = "cta",
                  label = "Call to Action",
                  label_field = "heading",
                  fields = {
                    { name = "heading", type = "text", required = true },
                    { name = "body", type = "textarea" },
                    { name = "button_text", type = "text", required = true },
                    { name = "button_url", type = "text", required = true },
                  },
                },
              },
            },
          },
        },
      },
    },
    -- Tabs: organizes page settings into Display and SEO tabs
    {
      name = "page_settings",
      type = "tabs",
      tabs = {
        {
          label = "Display",
          description = "Control how this page appears on the site",
          fields = {
            {
              name = "featured_image",
              type = "upload",
              relationship = { collection = "media" },
              admin = {
                description = "Hero image displayed at the top of the page",
              },
            },
            {
              name = "show_title",
              type = "checkbox",
              default_value = true,
              admin = {
                label = "Show Title",
                description = "Display the page title as an H1 heading",
              },
            },
            {
              name = "page_template",
              type = "select",
              default_value = "default",
              options = {
                { label = "Default", value = "default" },
                { label = "Full Width", value = "full_width" },
                { label = "Sidebar", value = "sidebar" },
                { label = "Landing", value = "landing" },
              },
              admin = {
                label = "Page Template",
                description = "Layout template for this page",
                width = "half",
              },
            },
          },
        },
        {
          label = "SEO",
          description = "Search engine optimization settings",
          fields = {
            {
              name = "meta_title",
              type = "text",
              localized = true,
              admin = {
                label = "Meta Title",
                placeholder = "Custom SEO title...",
              },
            },
            {
              name = "meta_description",
              type = "textarea",
              localized = true,
              admin = {
                label = "Meta Description",
                placeholder = "Describe this page for search engines...",
              },
            },
            {
              name = "og_image",
              type = "upload",
              relationship = { collection = "media" },
              admin = {
                label = "Social Image",
                description = "Image shown when shared on social media (1200x630 recommended)",
              },
            },
            {
              name = "no_index",
              type = "checkbox",
              default_value = false,
              admin = {
                label = "No Index",
                description = "Hide this page from search engines",
              },
            },
          },
        },
      },
    },
  },
  access = {
    read = "access.anyone",
    create = "access.editor_or_admin",
    update = "access.editor_or_admin",
    delete = "access.admin_only",
  },
})

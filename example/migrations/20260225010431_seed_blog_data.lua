local M = {}

function M.up()
  crap.log.info("Seeding Six Seven blog data...")

  -- Find or create the admin user
  -- crap.collections.find returns { documents = [...], total = N }
  local result = crap.collections.find("users", {})
  local admin_id = nil
  if result and result.documents and #result.documents > 0 then
    admin_id = result.documents[1].id
  else
    -- No user yet — create one. Uses crap.collections.create which handles
    -- password hashing for auth collections.
    local ok, admin = pcall(crap.collections.create, "users", {
      email = "admin@sixseven.blog",
      password = "admin123",
      name = "Admin",
      role = "admin",
    })
    if ok then
      admin_id = admin.id
      crap.log.info("Created admin user: " .. tostring(admin_id))
    else
      crap.log.warn("Could not create admin user: " .. tostring(admin))
      crap.log.warn("Posts will have no author — create a user manually")
    end
  end

  -- ── Categories ──────────────────────────────────────────────
  local cat_dev = crap.collections.create("categories", {
    title = "Development",
    slug = "development",
    description = "Programming tutorials, code deep-dives, and engineering insights.",
  })
  crap.log.info("Created category: " .. cat_dev.id)

  local cat_design = crap.collections.create("categories", {
    title = "Design",
    slug = "design",
    description = "Visual design, typography, and user experience.",
  })

  local cat_essay = crap.collections.create("categories", {
    title = "Essays",
    slug = "essays",
    description = "Long-form thinking about technology, culture, and craft.",
  })

  -- ── Tags ────────────────────────────────────────────────────
  local tag_rust = crap.collections.create("tags", {
    title = "Rust",
    slug = "rust",
  })

  local tag_lua = crap.collections.create("tags", {
    title = "Lua",
    slug = "lua",
  })

  local tag_cms = crap.collections.create("tags", {
    title = "CMS",
    slug = "cms",
  })

  local tag_web = crap.collections.create("tags", {
    title = "Web",
    slug = "web",
  })

  local tag_oss = crap.collections.create("tags", {
    title = "Open Source",
    slug = "open-source",
  })

  local tag_perf = crap.collections.create("tags", {
    title = "Performance",
    slug = "performance",
  })

  local tag_typography = crap.collections.create("tags", {
    title = "Typography",
    slug = "typography",
  })

  -- ── Posts ────────────────────────────────────────────────────

  -- Post 1: Why We Built Six Seven
  crap.collections.create("posts", {
    title = "Why We Built Six Seven",
    slug = "why-we-built-six-seven",
    author = admin_id,
    excerpt = "Every CMS makes you choose between flexibility and simplicity. We chose both. Here is why six and seven belong together.",
    _status = "published",
    published_at = "2026-01-15T09:00:00Z",
    category = cat_essay.id,
    tags = { tag_cms.id, tag_oss.id, tag_web.id },
    content = {
      {
        _block_type = "richtext",
        body = "<p>There is a number between six and eight. Everyone knows it. But few understand why it matters.</p><p>When we started building this blog, we wanted something that felt like writing in a notebook — no friction, no ceremony — but with the power of a real content system underneath. Most tools force you to pick a lane: either you get a beautiful editor with zero flexibility, or you get infinite config files and a blank terminal.</p><p>Six Seven is the space in between.</p>",
      },
      {
        _block_type = "quote",
        text = "The best tools disappear. You stop thinking about the tool and start thinking about the work.",
        attribution = "Frank Chimero",
      },
      {
        _block_type = "richtext",
        body = "<p>Our stack is deliberately simple: Rust for the core, Lua for the hooks, SQLite for the data, HTMX for the admin. No JavaScript build step. No container orchestration. One binary, one config directory, done.</p><h2>What makes it different</h2><p>Every collection is defined in a Lua file. Want to add a field? Edit a Lua table and restart. Want custom validation? Write a hook function. Want to change the access rules? One function per operation. The schema lives in code, not in a database migration maze.</p><p>We think the number seven has been overlooked for too long. This blog is our small correction.</p>",
      },
    },
    seo__meta_title = "Why We Built Six Seven — A New Kind of CMS",
    seo__meta_description = "Every CMS makes you choose between flexibility and simplicity. Six Seven chose both.",
  })

  -- Post 2: Lua Hooks Explained
  crap.collections.create("posts", {
    title = "Lua Hooks: The Extension System You Already Know",
    slug = "lua-hooks-explained",
    author = admin_id,
    excerpt = "How Six Seven uses Lua hooks for validation, transformation, and access control — without plugins, without complexity.",
    _status = "published",
    published_at = "2026-01-22T10:30:00Z",
    category = cat_dev.id,
    tags = { tag_lua.id, tag_cms.id },
    content = {
      {
        _block_type = "richtext",
        body = "<p>If you have ever configured Neovim, you already know the pattern. A Lua file defines behavior. A function runs at the right moment. Data flows in, transformed data flows out.</p><p>Six Seven hooks work the same way. Every lifecycle event — before validate, before change, after change, before delete — can be intercepted with a plain Lua function.</p>",
      },
      {
        _block_type = "code",
        language = "lua",
        code = '--- Auto-generate a slug from the title field.\nreturn function(value, context)\n    if value and value ~= "" then\n        return value\n    end\n    local title = context.data and context.data.title\n    if not title or title == "" then\n        return value\n    end\n    return title:lower()\n        :gsub("[^%w%s-]", "")\n        :gsub("%s+", "-")\n        :gsub("-+", "-")\n        :gsub("^-|-$", "")\nend',
      },
      {
        _block_type = "richtext",
        body = "<p>This is a real hook from the Six Seven blog. It runs on the <code>slug</code> field before validation. If no slug is provided, it generates one from the title. That is it. No plugin API, no registration ceremony, no abstract factory pattern.</p><h2>Access control</h2><p>The same pattern works for access control. Each operation (read, create, update, delete) gets a function that receives the current user and returns true, false, or a query constraint:</p>",
      },
      {
        _block_type = "code",
        language = "lua",
        code = '--- Only published posts are visible to anonymous users.\n--- Authors can see their own drafts.\nreturn function(context)\n    if context.user and context.user.role == "admin" then\n        return true\n    end\n    if context.user then\n        return {\n            { _or = {\n                { field = "_status", operator = "equals", value = "published" },\n                { field = "author", operator = "equals", value = context.user.id },\n            }}\n        }\n    end\n    return { { field = "_status", operator = "equals", value = "published" } }\nend',
      },
      {
        _block_type = "richtext",
        body = "<p>The access function returns a filter — the CMS merges it into the query. No data ever leaks because the constraint is applied at the database level. You cannot accidentally forget to check permissions in a template.</p>",
      },
    },
    seo__meta_title = "Lua Hooks Explained — Six Seven CMS",
    seo__meta_description = "How Lua hooks replace plugins with plain functions for validation, transformation, and access control.",
  })

  -- Post 3: Performance post
  crap.collections.create("posts", {
    title = "SQLite Is Enough",
    slug = "sqlite-is-enough",
    author = admin_id,
    excerpt = "Why we chose SQLite over Postgres, how WAL mode handles concurrency, and what the benchmarks actually show.",
    _status = "published",
    published_at = "2026-02-01T08:00:00Z",
    category = cat_dev.id,
    tags = { tag_rust.id, tag_perf.id },
    content = {
      {
        _block_type = "richtext",
        body = "<p>The first question everyone asks: <em>why SQLite?</em> You are building a CMS. CMS means Postgres. Or MySQL. Or at least some managed database with a connection string and a monthly bill.</p><p>We disagree.</p><h2>The numbers</h2><p>SQLite in WAL mode handles thousands of concurrent readers with zero coordination. Writes are serialized, yes — but a single write to an indexed table takes microseconds. Our content API benchmarks at 15,000 reads/second and 3,000 writes/second on commodity hardware. For a CMS, that is more than enough.</p>",
      },
      {
        _block_type = "quote",
        text = "SQLite is not a toy database. It is the most deployed database engine in the world.",
        attribution = "D. Richard Hipp",
      },
      {
        _block_type = "richtext",
        body = "<h2>Zero operational burden</h2><p>No connection pooling. No vacuum jobs. No replication lag. No credentials rotation. The database is a single file in your config directory. Back it up with <code>cp</code>. Move it with <code>scp</code>. Version it with git-lfs if you want.</p><p>The CMS creates one table per collection, with typed columns generated from your Lua field definitions. When you change a field, it runs <code>ALTER TABLE</code> on startup. No migration files for schema changes — the Lua file <em>is</em> the schema.</p><p>For most blogs, most marketing sites, most documentation portals — SQLite is not just enough. It is better.</p>",
      },
    },
    seo__meta_description = "Why SQLite with WAL mode outperforms Postgres for content management workloads.",
  })

  -- Post 4: Typography post
  crap.collections.create("posts", {
    title = "Seven Rules for Web Typography",
    slug = "seven-rules-web-typography",
    author = admin_id,
    excerpt = "A practical guide to making text look good on screens. No theory, no history — just seven rules that work.",
    _status = "published",
    published_at = "2026-02-10T12:00:00Z",
    category = cat_design.id,
    tags = { tag_typography.id, tag_web.id },
    content = {
      {
        _block_type = "richtext",
        body = "<p>Good typography is invisible. Bad typography is everywhere. Here are seven rules we follow on this blog and in the Six Seven admin UI.</p><h2>1. One typeface is enough</h2><p>Geist. That is it. One family, four weights (400, 500, 600, 700). If you need visual hierarchy, change the size or weight — not the font.</p><h2>2. Line length matters more than font size</h2><p>Keep body text between 45 and 75 characters per line. On this blog, that means <code>max-width: 42rem</code> on the content container.</p><h2>3. Vertical rhythm is real</h2><p>Set your line height to 1.6 for body text. Use multiples of your base spacing unit for margins. The eye notices when things do not line up, even if the brain cannot articulate why.</p>",
      },
      {
        _block_type = "richtext",
        body = "<h2>4. Contrast is not negotiable</h2><p>WCAG AA minimum: 4.5:1 for body text, 3:1 for large text. Test with a tool, not your eyes. Your monitor is lying to you.</p><h2>5. Do not center body text</h2><p>Left-aligned. Always. Centered text is for headings, toasts, and wedding invitations.</p><h2>6. Whitespace is not wasted space</h2><p>When in doubt, add more padding. Dense layouts feel cheap. Generous spacing feels intentional.</p><h2>7. Load fonts properly</h2><p>Static font files, not variable fonts (for now). <code>font-display: swap</code> in every <code>@font-face</code> rule. Preload the regular weight. Let the browser handle the rest.</p>",
      },
    },
    seo__meta_title = "Seven Rules for Web Typography — Six Seven",
  })

  -- Post 5: Draft post (showcases versioning)
  crap.collections.create("posts", {
    title = "Building a CLI That Feels Right",
    slug = "building-a-cli-that-feels-right",
    author = admin_id,
    excerpt = "Interactive prompts, sensible defaults, and the --no-input escape hatch. How we designed the crap-cms command line.",
    _status = "draft",
    category = cat_dev.id,
    tags = { tag_rust.id, tag_cms.id, tag_oss.id },
    content = {
      {
        _block_type = "richtext",
        body = "<p>A good CLI does two things well: it gets out of your way when you know what you want, and it guides you when you do not.</p><p>The Six Seven CLI follows a simple rule: if you provide all the flags, it runs silently. If you omit them, it asks. No guessing, no heuristics — just a <code>--no-input</code> flag for scripts and CI.</p>",
      },
      {
        _block_type = "code",
        language = "shell",
        code = '# Interactive — prompts for everything\ncrap-cms make collection ./my-site\n\n# Scripted — flags only, no prompts\ncrap-cms make collection ./my-site posts \\\n  --fields "title:text:required,slug:text:required,body:richtext" \\\n  --versions --no-input',
      },
      {
        _block_type = "richtext",
        body = "<p>This post is a draft. It will be published once we finalize the CLI documentation. The versioning system tracks every save, so nothing is lost.</p>",
      },
    },
  })

  -- ── Pages ───────────────────────────────────────────────────

  crap.collections.create("pages", {
    title = "About",
    slug = "about",
    _status = "published",
    content = {
      {
        _block_type = "richtext",
        body = "<h1>About Six Seven</h1><p>Six Seven is a blog about the things between. Between six and eight. Between simple and powerful. Between writing code and writing prose.</p><p>We believe the best tools are the ones you forget you are using. Our CMS is a single binary. Our database is a single file. Our extensions are plain Lua functions. No build steps, no containers, no monthly bills.</p>",
      },
      {
        _block_type = "cta",
        heading = "Want to try it?",
        body = "Six Seven is open source. Clone the repo, run one command, and start writing.",
        button_text = "View on GitHub",
        button_url = "https://github.com/sixseven/cms",
      },
    },
    seo__meta_title = "About — Six Seven",
    seo__meta_description = "Six Seven is a blog about building simple, powerful tools. Powered by a headless CMS written in Rust.",
  })

  crap.collections.create("pages", {
    title = "Contact",
    slug = "contact",
    _status = "published",
    content = {
      {
        _block_type = "richtext",
        body = "<h1>Get in Touch</h1><p>Have a question, found a bug, or just want to say hello?</p><p>Email us at <strong>hello@sixseven.blog</strong> or open an issue on GitHub. We read everything.</p>",
      },
    },
    seo__meta_title = "Contact — Six Seven",
  })

  -- ── Global: Site Settings ───────────────────────────────────
  crap.globals.update("site_settings", {
    site_name = "Six Seven",
    tagline = "Where six meets seven",
    description = "A blog about building simple, powerful tools. Code, design, and the spaces in between.",
    social__github = "https://github.com/sixseven",
    social__twitter = "https://x.com/sixsevenblog",
    social__mastodon = "https://mastodon.social/@sixseven",
    seo__default_title_suffix = " | Six Seven",
  })

  crap.log.info("Seed complete: 3 categories, 7 tags, 5 posts, 2 pages, site settings")
end

function M.down()
  -- Delete seeded content in reverse order
  local function delete_all(collection)
    local result = crap.collections.find(collection, {})
    if result and result.documents then
      for _, doc in ipairs(result.documents) do
        crap.collections.delete(collection, doc.id)
      end
    end
  end

  delete_all("posts")
  delete_all("pages")
  delete_all("tags")
  delete_all("categories")
  -- Note: users not deleted here (access control prevents it in migration context)

  crap.globals.update("site_settings", {
    site_name = "",
    tagline = "",
    description = "",
  })

  crap.log.info("Seed data removed")
end

return M

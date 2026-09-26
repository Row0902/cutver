## [{{ tag }}] - {{ date }}

{%- if breaking %}
### ⚠️ Breaking Changes
{% for c in commits if c.is_breaking -%}
- {{ ('**' ~ c.scope ~ '**: ') if c.scope }}{{ c.clean_description or c.description }}{% if c.pr_number %} in [#{{ c.pr_number }}]({{ c.pr_url }}){% endif %}{% if c.short_hash and c.commit_url %} ([{{ c.short_hash }}]({{ c.commit_url }})){% elif c.short_hash %} ({{ c.short_hash }}){% endif %}{% if c.author %} by @{{ c.author }}{% endif %}
{% endfor %}
{%- endif %}

{%- if features %}
### 🚀 Features & Enhancements
{% for c in commits if c.commit_type == 'feat' and not c.is_breaking -%}
- {{ ('**' ~ c.scope ~ '**: ') if c.scope }}{{ c.clean_description or c.description }}{% if c.pr_number %} in [#{{ c.pr_number }}]({{ c.pr_url }}){% endif %}{% if c.short_hash and c.commit_url %} ([{{ c.short_hash }}]({{ c.commit_url }})){% elif c.short_hash %} ({{ c.short_hash }}){% endif %}{% if c.author %} by @{{ c.author }}{% endif %}
{% endfor %}
{%- endif %}

{%- if fixes %}
### 🐛 Bug Fixes
{% for c in commits if c.commit_type == 'fix' and not c.is_breaking -%}
- {{ ('**' ~ c.scope ~ '**: ') if c.scope }}{{ c.clean_description or c.description }}{% if c.pr_number %} in [#{{ c.pr_number }}]({{ c.pr_url }}){% endif %}{% if c.short_hash and c.commit_url %} ([{{ c.short_hash }}]({{ c.commit_url }})){% elif c.short_hash %} ({{ c.short_hash }}){% endif %}{% if c.author %} by @{{ c.author }}{% endif %}
{% endfor %}
{%- endif %}

{%- if perf %}
### ⚡ Performance Improvements
{% for c in commits if c.commit_type == 'perf' and not c.is_breaking -%}
- {{ ('**' ~ c.scope ~ '**: ') if c.scope }}{{ c.clean_description or c.description }}{% if c.pr_number %} in [#{{ c.pr_number }}]({{ c.pr_url }}){% endif %}{% if c.short_hash and c.commit_url %} ([{{ c.short_hash }}]({{ c.commit_url }})){% elif c.short_hash %} ({{ c.short_hash }}){% endif %}{% if c.author %} by @{{ c.author }}{% endif %}
{% endfor %}
{%- endif %}

{%- if refactor %}
### 🔄 Code Refactoring
{% for c in commits if c.commit_type == 'refactor' and not c.is_breaking -%}
- {{ ('**' ~ c.scope ~ '**: ') if c.scope }}{{ c.clean_description or c.description }}{% if c.pr_number %} in [#{{ c.pr_number }}]({{ c.pr_url }}){% endif %}{% if c.short_hash and c.commit_url %} ([{{ c.short_hash }}]({{ c.commit_url }})){% elif c.short_hash %} ({{ c.short_hash }}){% endif %}{% if c.author %} by @{{ c.author }}{% endif %}
{% endfor %}
{%- endif %}

{%- if docs %}
### 📚 Documentation
{% for c in commits if c.commit_type == 'docs' and not c.is_breaking -%}
- {{ ('**' ~ c.scope ~ '**: ') if c.scope }}{{ c.clean_description or c.description }}{% if c.pr_number %} in [#{{ c.pr_number }}]({{ c.pr_url }}){% endif %}{% if c.short_hash and c.commit_url %} ([{{ c.short_hash }}]({{ c.commit_url }})){% elif c.short_hash %} ({{ c.short_hash }}){% endif %}{% if c.author %} by @{{ c.author }}{% endif %}
{% endfor %}
{%- endif %}

{%- if maintenance %}
### 🛠️ Maintenance & Dependencies
{% for c in commits if c.commit_type in ['chore', 'build', 'ci', 'test'] and not c.is_breaking -%}
- {{ ('**' ~ c.scope ~ '**: ') if c.scope }}{{ c.clean_description or c.description }}{% if c.pr_number %} in [#{{ c.pr_number }}]({{ c.pr_url }}){% endif %}{% if c.short_hash and c.commit_url %} ([{{ c.short_hash }}]({{ c.commit_url }})){% elif c.short_hash %} ({{ c.short_hash }}){% endif %}{% if c.author %} by @{{ c.author }}{% endif %}
{% endfor %}
{%- endif %}

{%- if other %}
### 📦 Other Changes
{{ other }}
{%- endif %}

{%- if contributors %}
### 👥 Contributors
{% for author in contributors -%}
- @{{ author }}
{% endfor %}
{%- endif %}

{%- if compare_url %}
---
**Full Changelog**: {{ compare_url }}
{%- endif %}

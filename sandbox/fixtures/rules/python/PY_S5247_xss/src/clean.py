# Clean: Safe template
from jinja2 import Template
def render_page(user_input):
    template = Template("{{ user_input }}")
    return template.render(user_input=user_input)

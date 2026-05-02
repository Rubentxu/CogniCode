# Smelly: XSS in template
from jinja2 import Template
def render_page(user_input):
    template = Template("{{ user_input | safe }}")
    return template.render(user_input=user_input)

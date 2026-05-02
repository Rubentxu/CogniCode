# Smelly: XXE vulnerable XML parse
from lxml import etree
def parse_xml(path):
    return etree.parse(path)

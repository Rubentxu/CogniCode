# Clean: Strong TLS protocol
import ssl
context = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)

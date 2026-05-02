# Clean: Strong cipher
from Crypto.Cipher import AES
def encrypt(data, key):
    cipher = AES.new(key, AES.MODE_GCM)
    return cipher.encrypt(data)

# Smelly: Weak cipher
from Crypto.Cipher import DES
def encrypt(data, key):
    cipher = DES.new(key, DES.MODE_ECB)
    return cipher.encrypt(data)

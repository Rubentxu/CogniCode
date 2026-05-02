# Smelly: Deeply nested ifs
def check_values(a, b, c, d, e, f):
    if a:
        if b:
            if c:
                if d:
                    if e:
                        if f:
                            return True
    return False

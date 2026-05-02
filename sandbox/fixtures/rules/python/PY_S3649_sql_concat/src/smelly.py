# Smelly: SQL with string concatenation
def get_user(table, user_id):
    query = "SELECT * FROM " + table + " WHERE id = " + user_id
    return query

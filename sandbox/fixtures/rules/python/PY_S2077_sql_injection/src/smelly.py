# Smelly: SQL injection via f-string
def get_user(table, user_id):
    query = f"SELECT * FROM {table} WHERE id = {user_id}"
    return query

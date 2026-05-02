# Clean: Parameterized query
def get_user(user_id):
    query = "SELECT * FROM users WHERE id = ?"
    return query, (user_id,)

# multi-lang-types fixture - Python type stubs
# These Python type stubs mirror the Rust domain models
# Used by type-ref walkers to test cross-language type reference resolution

from datetime import datetime
from enum import Enum
from typing import Dict, Optional, List, Protocol, Any, Callable

class UserId:
    """User identifier - corresponds to Rust's UserId struct"""
    def __init__(self, value: str) -> None:
        self._value = value

    @property
    def value(self) -> str:
        return self._value

    def __str__(self) -> str:
        return self._value

    def __repr__(self) -> str:
        return f"UserId({self._value!r})"

    def __eq__(self, other: object) -> bool:
        if isinstance(other, UserId):
            return self._value == other._value
        return False

    def __hash__(self) -> int:
        return hash(self._value)


class Email:
    """Email address value object"""
    def __init__(self, address: str) -> None:
        if "@" not in address:
            raise ValueError(f"Invalid email address: {address}")
        self._address = address

    @property
    def address(self) -> str:
        return self._address

    def __str__(self) -> str:
        return self._address

    def __repr__(self) -> str:
        return f"Email({self._address!r})"


class UserRole(Enum):
    """User role enumeration - mirrors Rust's UserRole enum"""
    ADMIN = "admin"
    EDITOR = "editor"
    VIEWER = "viewer"
    GUEST = "guest"


class User:
    """User aggregate root - mirrors Rust's User struct"""
    def __init__(
        self,
        id: UserId,
        email: Email,
        name: str,
        role: UserRole,
        metadata: Dict[str, str],
        created_at: datetime,
    ) -> None:
        self.id = id
        self.email = email
        self.name = name
        self.role = role
        self.metadata = metadata
        self.created_at = created_at

    def to_dict(self) -> Dict[str, Any]:
        return {
            "id": str(self.id),
            "email": str(self.email),
            "name": self.name,
            "role": self.role.value,
            "metadata": self.metadata,
            "created_at": self.created_at.isoformat(),
        }


class RepositoryError(Exception):
    """Repository error - mirrors Rust's RepositoryError enum"""
    class NotFound(Exception):
        pass

    class ValidationError(Exception):
        pass

    class DatabaseError(Exception):
        pass

    class ConcurrentModification(Exception):
        pass


class Repository(Protocol):
    """Repository trait - mirrors Rust's Repository<T> trait"""
    def find_by_id(self, id: str) -> Optional[User]:
        ...

    def find_all(self) -> List[User]:
        ...

    def save(self, user: User) -> None:
        ...

    def delete(self, id: str) -> None:
        ...


class PostgresRepository:
    """PostgreSQL repository - mirrors Rust's PostgresRepository"""
    def __init__(self, connection_string: str) -> None:
        self._conn_str = connection_string

    def find_by_id(self, id: str) -> Optional[User]:
        return None

    def find_all(self) -> List[User]:
        return []

    def save(self, user: User) -> None:
        pass

    def delete(self, id: str) -> None:
        pass


class UserService:
    """User service - mirrors Rust's UserService<R>"""
    def __init__(
        self,
        repository: Repository,
        email_sender: EmailSender,
    ) -> None:
        self._repository = repository
        self._email_sender = email_sender

    def create_user(self, email: Email, name: str) -> User:
        if not name:
            raise UserServiceError.ValidationError("Name cannot be empty")
        # Implementation here
        ...

    def find_user(self, id: str) -> Optional[User]:
        return None


class EmailSender(Protocol):
    """Email sender interface - mirrors Rust's EmailSender trait"""
    def send(self, to: Email, subject: str, body: str) -> None:
        ...


class Page(Generic[T]):
    """Pagination wrapper - mirrors Rust's Page<T>"""
    def __init__(
        self,
        items: List[T],
        total: int,
        page: int,
        page_size: int,
    ) -> None:
        self.items = items
        self.total = total
        self.page = page
        self.page_size = page_size

    def total_pages(self) -> int:
        return (self.total + self.page_size - 1) // self.page_size

    def has_next(self) -> bool:
        return self.page < self.total_pages()

    def has_prev(self) -> bool:
        return self.page > 1

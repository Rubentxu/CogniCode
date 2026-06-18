// multi-lang-types fixture - Go types
// These Go types mirror the Rust domain models

package domain

import (
	"context"
	"time"
)

// UserId is a strongly-typed user identifier
type UserId struct {
	value string
}

func NewUserId(value string) *UserId {
	return &UserId{value: value}
}

func (u *UserId) String() string { return u.value }
func (u *UserId) Equals(other *UserId) bool { return u.value == other.value }

// Email is a value object for email addresses
type Email struct {
	address string
}

func NewEmail(address string) (*Email, error) {
	if address == "" || !contains(address, "@") {
		return nil, &ValidationError{Field: "email", Message: "invalid email address"}
	}
	return &Email{address: address}, nil
}

func (e *Email) Address() string { return e.address }
func (e *Email) String() string    { return e.address }

// UserRole represents the user's access level
type UserRole int

const (
	RoleAdmin  UserRole = iota
	RoleEditor
	RoleViewer
	RoleGuest
)

func (r UserRole) String() string {
	switch r {
	case RoleAdmin:
		return "admin"
	case RoleEditor:
		return "editor"
	case RoleViewer:
		return "viewer"
	case RoleGuest:
		return "guest"
	default:
		return "unknown"
	}
}

// User is the aggregate root for user management
type User struct {
	Id        *UserId
	Email     *Email
	Name      string
	Role      UserRole
	Metadata  map[string]string
	CreatedAt time.Time
}

// UserService provides user business logic
type UserService struct {
	repo         Repository[*User]
	emailSender  EmailSender
}

func NewUserService(repo Repository[*User], emailSender EmailSender) *UserService {
	return &UserService{
		repo:        repo,
		emailSender: emailSender,
	}
}

func (s *UserService) CreateUser(ctx context.Context, email *Email, name string) (*User, error) {
	if name == "" {
		return nil, &ValidationError{Field: "name", Message: "name cannot be empty"}
	}

	user := &User{
		Id:        NewUserId(generateUUID()),
		Email:     email,
		Name:      name,
		Role:      RoleViewer,
		Metadata:  make(map[string]string),
		CreatedAt: time.Now().UTC(),
	}

	if err := s.repo.Save(ctx, user); err != nil {
		return nil, err
	}

	return user, nil
}

func (s *UserService) FindUser(ctx context.Context, id string) (*User, error) {
	return s.repo.FindById(ctx, id)
}

func (s *UserService) FindUsersByRole(ctx context.Context, role UserRole) ([]*User, error) {
	all, err := s.repo.FindAll(ctx)
	if err != nil {
		return nil, err
	}

	var result []*User
	for _, u := range all {
		if u.Role == role {
			result = append(result, u)
		}
	}
	return result, nil
}

// Repository defines the persistence contract
type Repository[T any] interface {
	FindById(ctx context.Context, id string) (*T, error)
	FindAll(ctx context.Context) ([]*T, error)
	Save(ctx context.Context, entity *T) error
	Delete(ctx context.Context, id string) error
}

// RepositoryError represents data access failures
type RepositoryError struct {
	Kind    string
	Message string
}

func (e *RepositoryError) Error() string {
	return e.Kind + ": " + e.Message
}

var (
	ErrNotFound               = &RepositoryError{Kind: "not-found"}
	ErrConcurrentModification = &RepositoryError{Kind: "concurrent-modification"}
)

// PostgresRepository implements Repository for User entities
type PostgresRepository struct {
	connectionString string
}

func NewPostgresRepository(connStr string) *PostgresRepository {
	return &PostgresRepository{connectionString: connStr}
}

func (r *PostgresRepository) FindById(ctx context.Context, id string) (*User, error) {
	return nil, ErrNotFound
}

func (r *PostgresRepository) FindAll(ctx context.Context) ([]*User, error) {
	return []*User{}, nil
}

func (r *PostgresRepository) Save(ctx context.Context, user *User) error {
	return nil
}

func (r *PostgresRepository) Delete(ctx context.Context, id string) error {
	return nil
}

// EmailSender defines the email sending contract
type EmailSender interface {
	Send(ctx context.Context, to *Email, subject, body string) error
}

// EmailError represents email delivery failures
type EmailError struct {
	Kind    string
	Message string
}

func (e *EmailError) Error() string {
	return e.Kind + ": " + e.Message
}

// Page represents a paginated result set
type Page[T any] struct {
	Items      []T
	Total      uint64
	Page       uint32
	PageSize   uint32
	TotalPages uint32
	HasNext    bool
	HasPrev    bool
}

func NewPage[T any](items []T, total uint64, page uint32, pageSize uint32) *Page[T] {
	totalPages := uint32((total + uint64(pageSize) - 1) / uint64(pageSize))
	return &Page[T]{
		Items:      items,
		Total:      total,
		Page:       page,
		PageSize:   pageSize,
		TotalPages: totalPages,
		HasNext:    page < totalPages,
		HasPrev:    page > 1,
	}
}

// ValidationError represents input validation failures
type ValidationError struct {
	Field   string
	Message string
}

func (e *ValidationError) Error() string {
	return e.Field + ": " + e.Message
}

// Helpers
func contains(s, substr string) bool {
	return len(s) >= len(substr) && (s == substr || len(s) > 0 && containsImpl(s, substr))
}

func containsImpl(s, substr string) bool {
	for i := 0; i <= len(s)-len(substr); i++ {
		if s[i:i+len(substr)] == substr {
			return true
		}
	}
	return false
}

func generateUUID() string {
	// Simple UUID v4 generator stub
	return "00000000-0000-0000-0000-000000000000"
}

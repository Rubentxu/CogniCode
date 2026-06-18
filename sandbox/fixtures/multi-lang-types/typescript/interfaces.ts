// multi-lang-types fixture - TypeScript interfaces
// These TypeScript interfaces mirror the Rust domain models

export interface UserId {
  readonly value: string;
}

export interface Email {
  readonly address: string;
}

export enum UserRole {
  Admin = "admin",
  Editor = "editor",
  Viewer = "viewer",
  Guest = "guest",
}

export interface User {
  readonly id: UserId;
  readonly email: Email;
  readonly name: string;
  readonly role: UserRole;
  readonly metadata: Record<string, string>;
  readonly createdAt: Date;
}

export class UserIdImpl implements UserId {
  constructor(readonly value: string) {}
  toString(): string { return this.value; }
  equals(other: UserId): boolean { return this.value === other.value; }
}

export class EmailImpl implements Email {
  constructor(readonly address: string) {
    if (!address.includes("@")) {
      throw new Error(`Invalid email address: ${address}`);
    }
  }
  toString(): string { return this.address; }
}

export interface Repository<T> {
  findById(id: string): Promise<T | null>;
  findAll(): Promise<T[]>;
  save(entity: T): Promise<void>;
  delete(id: string): Promise<void>;
}

export type RepositoryError =
  | { kind: "not-found"; message: string }
  | { kind: "validation-error"; message: string }
  | { kind: "database-error"; message: string }
  | { kind: "concurrent-modification" };

export interface PostgresRepository extends Repository<User> {
  connectionString: string;
  withPool(pool: unknown): PostgresRepository;
}

export interface UserService<R extends Repository<User> = Repository<User>> {
  createUser(email: Email, name: string): Promise<User>;
  findUser(id: string): Promise<User | null>;
  findUsersByRole(role: UserRole): Promise<User[]>;
}

export interface EmailSender {
  send(to: Email, subject: string, body: string): Promise<void>;
}

export type EmailError =
  | { kind: "smtp-error"; message: string }
  | { kind: "invalid-recipient"; address: string };

export interface Page<T> {
  readonly items: T[];
  readonly total: number;
  readonly page: number;
  readonly pageSize: number;
  readonly totalPages: number;
  readonly hasNext: boolean;
  readonly hasPrev: boolean;
}

export function createPage<T>(
  items: T[],
  total: number,
  page: number,
  pageSize: number
): Page<T> {
  const totalPages = Math.ceil(total / pageSize);
  return {
    items,
    total,
    page,
    pageSize,
    totalPages,
    hasNext: page < totalPages,
    hasPrev: page > 1,
  };
}

export interface UserServiceError {
  readonly type: "validation-error" | "repository-error" | "email-error";
  readonly message: string;
}

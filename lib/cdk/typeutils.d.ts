// Utility type modifiers.

/**
 * An opaque type that can be used to distinguish different things that
 * extends the same primitive types when they are conceptually different
 * things, like lat and long, they are both numbers but are actually
 * different things.
 *
 * # Example
 * ```ts
 * type Lat = Opaque<"Lat", number>;
 * type Long = Opaque<"Long", number>;
 * ```
 */
type Opaque<N, T> = T & { __opaque__: N };

type Primitive = number | boolean | string | null | undefined;

type DeepPartial<T> = {
  [P in keyof T]?: T[P] extends object ? DeepPartial<T[P]> : T[P];
};

type Mutable<T> = {
  -readonly [P in keyof T]: T[P];
};

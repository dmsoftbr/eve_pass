// EVEPass mobile — Vault list (scaffold). List-first layout (the desktop
// three-pane doesn't fit a phone): search on top, rows with quick copy, tap to
// open detail. Copy/TOTP go through the core; the vaultKey stays in the Session.
import { useEffect, useMemo, useState } from "react";
import { FlatList, Pressable, Text, TextInput, View } from "react-native";
import type { Session } from "../lib/core";
import { listItemViews, type ItemView } from "../lib/vault";

export function VaultScreen({ session, onOpen }: { session: Session; onOpen: (id: string) => void }) {
  const [items, setItems] = useState<ItemView[]>([]);
  const [query, setQuery] = useState("");

  useEffect(() => {
    listItemViews(session).then(setItems);
  }, [session]);

  const visible = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return items;
    return items.filter(
      (i) => i.title.toLowerCase().includes(q) || i.username.toLowerCase().includes(q),
    );
  }, [items, query]);

  return (
    <View style={{ flex: 1, paddingTop: 48 }}>
      <TextInput
        placeholder="Buscar…"
        value={query}
        onChangeText={setQuery}
        style={{ margin: 12, borderWidth: 1, borderColor: "#2a2a35", borderRadius: 10, padding: 12, color: "#fff" }}
      />
      <FlatList
        data={visible}
        keyExtractor={(i) => i.id}
        renderItem={({ item }) => (
          <Pressable
            onPress={() => onOpen(item.id)}
            style={{ paddingHorizontal: 16, paddingVertical: 12, borderBottomWidth: 1, borderColor: "#1a1a22" }}
          >
            <Text style={{ color: "#f5f5f5", fontSize: 15 }}>{item.title}</Text>
            <Text style={{ color: "#8a8a99", fontSize: 12 }}>{item.username || item.url}</Text>
          </Pressable>
        )}
      />
    </View>
  );
}

//! Horizontal collision layers and weapon target policy. Keep the initial,
//! rebuilt and refined colliders on the same layer.
use super::*;

pub(super) enum CollisionLayer {
    Solid,
    Tank,
    Water,
    Deck,
}

impl CollisionLayer {
    pub(super) fn groups(self) -> InteractionGroups {
        let (membership, filter) = match self {
            Self::Solid => (Group::GROUP_1, Group::GROUP_1 | Group::GROUP_2),
            Self::Tank => (
                Group::GROUP_2,
                Group::GROUP_1 | Group::GROUP_2 | Group::GROUP_3,
            ),
            Self::Water => (Group::GROUP_3, Group::GROUP_2),
            Self::Deck => return InteractionGroups::none(),
        };
        InteractionGroups::new(membership, filter, InteractionTestMode::And)
    }
}

impl Piece {
    pub(super) fn collision_layer(&self) -> CollisionLayer {
        if self.bridge_deck {
            CollisionLayer::Deck
        } else if self.material == Material::Launcher {
            CollisionLayer::Tank
        } else {
            CollisionLayer::Solid
        }
    }
}

impl World {
    /// Terrain barriers have no piece ID. All weapons pass above bridge decks;
    /// callers handle shooter exclusion and whether a hit can damage a fixture.
    pub(super) fn weapon_target(&self, collider: &Collider) -> Option<usize> {
        let id = usize::try_from(collider.user_data).ok()?.checked_sub(1)?;
        let piece = self.pieces.get(id)?;
        (piece.active
            && collider.is_enabled()
            && self
                .physics
                .bodies
                .get(piece.body)
                .is_some_and(|b| b.is_enabled())
            && !matches!(piece.collision_layer(), CollisionLayer::Deck))
        .then_some(id)
    }
}

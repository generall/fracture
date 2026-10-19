

Let's build a game I wanted to build for the last 10 years.


I am not a game developer, but I am a software engineer, I work mostly in Rust.

So the idea:

A 2d or isomeric view game with a view from the top about tanks/machines battle.


Kind of similar to TANKS game from the 90s, but with heavy emphasis on physics simulation and realistic object desrtruction.

Player should not be limited to a grid, and there should be no abstract "health points" for objects. Instead, each object should have a physical representation and can be damaged or destroyed based on the physics simulation.

I have even thought about itroducing a budget version of finite element analysis into the game to simulate how different materials respond to stress and impact. But that would be up to you to decide how to build something like this.

I don't think I want any textures to be used, everything should be renderred with just lines and shapes, giving it a minimalist and clean aesthetic. The focus should be on the mechanics and physics. I figured we can spend more resources on the physics engine instead of graphics.

The game ideally should work in browser and have a multiplayer mode, but that can be a later stage. For now, let's focus on building the physics. I want it to be enjoyable to destroy objects in this game.


We are starting project from scratch, so please start with a plan and choice of technologies.

I want to understand if there is already some capable enough physics engine we can leverage, or we have to build our own.



---

Feedback over simulation:


- The font is barelly visible on the page, it is too thin and non-contrasting.

- The interface looks like it is a bitrix admin panel, not a game.

- The tank feels like it is on ice, it slides back from the shooting. It is also round, which doesn't help. Give it two tracks and give control over them separatelly, but still convenient.

- the projectile feels like a soft ball. It should be a shell. Maybe even have a small explosion on impact.

- Projectile should not stay after impact, again it is not a ball.

- overall style is too bage. I was thinking something like https://d3kjluh73b9h9o.cloudfront.net/original/4X/5/7/1/57122e6aa2a050cb871ce80fc2cb0e6dcc6244cc.jpeg in terms of rendering only edges with dark background and light lines.

- The destruction of objects looks nice, but it is too square and too weak. Maybe make it more granular? Also if object if destroyed into small enough pieces, then should disapper.


---

- Zoom is not smooth enough

- still feels like tank have almost no mass and friction. It has very high inertia on rotation.

- when I shoot into a rock field, they all simply disintegrate, like a chain reaction. This can't happen in real life. What are the options to make it more realistic? Ideally I would expect that objects would fall apart into bigger chunks, and not into dust.


---

Next steps:


- In the editor I don't have a way to manipulate the mesh. It generates strange layout, which I don't think matches the expectation of the realistic object. For example it creates a wall with very pointy triangles and I can't do anything about it. As a result fragments of the wall looks like fiberglass instead of stones.


- I want to scale testing range. I want to see a multiple simulations of the same object being hit by projectile or smashed by a tank with different parameters - stiffness, grid, mass, etc. And I want to be able to pick the best one. So so this way we could iterate towards the best simulation parameters for the game. This would require a way to save and load different simulation states and compare them visually.


---


Let's start building the object library:


- barrel: a cylindrical shape that moves around easily, hard to be destroyed by shooting, but can be smashed by a tank if it presses it against a wall or another object.

- explosive barrel: similar to a barrel, but when damaged it explodes and damages nearby objects.

- a wall: doesn't move, can be destroyed by both shooting and smashing, do not move easily (e.g. heavy mass, high friction). Should break into larger chunks when hit by a tank and smaller pieces when hit by a projectile.

- a rock: can be destroyed by a lot of shooting, but not easily by smashing. Should gradually cheap into smaller pieces when hit by projectiles, but should not disintegrate completely unless hit by a very powerful force. Rocks should have irregular shapes and edges to make the destruction more realistic.

Each object should have its own color to be easily distinguishable in the game.


Build the basic version of the components and make them editable in the editor.
As the next iteration we will do testing and tuning of the physics parameters for each object to achieve the desired behavior in the game.

----

Implement concept of non-elastic deformation: an object can be deformed if enough force is applied, and it will not return to its original shape. This can be especially matching the behaviour of barrels and metal fences.


---

Please implement the concept of different ammunition: there should be:


- an explosive shell: causes a small explosion on impact, damaging nearby objects. Same as the current one
- an armor-piercing shell: larger kinetic energy, smaller explosion.
- a machine gun bullet: small kinetic energy, no explosion, high rate of fire, can damage light objects but not heavy ones.
- a laser - removes material at the hit point without applying a force: first is plits the object frame into smaller parts until the smallest grid achieved, then removes the tile from the grid one by one.

----


Update objects:

- Wall should be about as resistant as the "barier" object from the first example.

- Rock should be about as enfluenced by the projectiles, as the current wall. It also should be ancored to the ground in the middle, so the tank can't push it at all, only chip away with projectiles.


----

Please implement different terrain types:

- default ground - same as the current one

- road - allows faster movement, and overall better control of the tank.

- sand - slows down the tank.

- water - can't be crossed unless there is a bridge.

- Bridge have a control point on each side, and can be destroyed by shooting or smashing. When destroyed, the bridge will fall into the water and can't be crossed anymore. If tank is on the bridge when it is destroyed, it will fall into the water and be destroyed as well.


----


Please implement a level editor. In level editor we should be able to choose objects from the library (as well as terrain types, undestructable objects, spawn points, e.t.c.) and place them on the map. On level editor we don't only select objects from the library, but also should be able to change their shape. For example draw a very long wall, or change the size of a rock.

There should be a way to place a "special" object on the map, which will represent a team flag.
If a tank from the opposing team captures the flag, it will be considered a victory for that team.
Flag can't be destroyed by shooting, only touch of the tank can "capture" it.


---

Let's make level editor more convenient to use:

- move `draw unbreakable wall` to object, next to draw wall.

- make "drawing" instruments fill the grid instead of free drawing.

- when drawing a long wall, currenly it is only ancored in one place. This makes very long walls behave unreawlistically: on impact the whole wall seems to be affected. We need to limit the propagation of forces along the wall. One solution I see is to anchor the wall to the ground every several meters, so it won't fly away when hit by a tank or projectile. Please let me know about alternative solutions to this problem.

---











---

Please implement a "fog of war" mechanic: player should only be able to see those area which is currently falls into a cone of vision of the tank + a smaller circle around the tank. The code of vision goes where the barrel is pointing.

Cone of vision should interract with different objects differently, for example:

- Tank can't see through walls, rocks, or other solid objects.
- Tank can see through the barrels and fences.
- Tank can't see through tall grass, but can shoot through it and also move through it.
- e.t.c.

At the beginning of the level the player doesn't see where the objects. After seeing the object and then loosing it from the view, the player should only see the last known position of the object, but not its current position. This will add a layer of strategy to the game, as players will have to remember where they saw enemies and plan their movements accordingly.

Player should know the terrain layout of the map all the time, as it doesn't change during the game.

In the editor play the user should be able to turn off and on the fog of war mechanic to test the level and see how it looks like without it.



---


Please work on sound design for the game. It should include:

- default background music, which is kind of uplifting, like a marching music, but not too fast.
- Battle music, which is running when you see enemy. The transition between default and battle music should be smooth, and the battle music should be more intense and faster.

- Sounds for
  - shooting, different for different types of ammunition
  - tank movement
  - tank engine
  - tank collision with objects
  - projectile impact with objects
  - object destruction
  - e.t.c.

---

Please introduve a multiplayer over the local network into the game.

One player should become a host, and other players should be able to connect to the host by entering the host's IP address or auto-discovering it on the local network. The host should be able to select map and see who's conencted and joining which team. The host should also be able to start the game when all players are ready.

How I think it should work:

- All phisics simulation should be done on the host side, and all clients should only send their input to the host and receive the updated game state from the host.
- Use most efficient binary level protocol to send the game state or even diff, depending on what would be more efficint. Use UDP if needed, use all necessary compressions.
- Take advantage of the players visibility to reduce the amount of data sent to each player.
- Make all necessary optimizations to make sure the game runs smoothly on a local network with up to 6 players, even if the map is large and has a lot of objects.

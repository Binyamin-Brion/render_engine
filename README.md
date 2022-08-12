# render_engine

Overview
=========

A rendering engine that handles rendering objects as well as executing any logic associated with them. Such an object could be a star-
there is a visual representation of it, as well as the light it casts on other objects, and may spin as well. Providing the appropriate parameters to model
this object, this engine will render the object and its visual influence on the scene. The engine will also make sure the star rotates at the provided speed.

Sample Scene
=======
![Alt Text](https://github.com/Binyamin-Brion/render_engine/blob/master/sample_scene/sample_scene_space.png)

Features
==========

Object Rendering
-------------------

* Deferred rendering for reduced cost of lighting calculations on the GPU compared to forward rendering
* Specify render systems to allow different shader programs to execute on the same objects
* Calculation of required GPU resources to minimize waste of resources

Object Logic
-----------------

* Entity Component System for cache friendly object data storage
* Multi-threaded execution of object logic
* Store object position within the world as a hashmap, providing quick operations to ensure only visible objects have their logic executd

Debugging
------------

* Playback functionality. Upon exiting the program either through a graceful exit or unexpected termination, play back
the history leading to the exit. Additionally is the ability to detach the camera from th playback and observe the scene from
a different angle while viewing a historical scene. Furthermore is the ability to continue the playback past the last stored frame to
see if any changes made in code had the desired effect.

Technologies Used
===============

* OpenGL 4.5
* Rust
